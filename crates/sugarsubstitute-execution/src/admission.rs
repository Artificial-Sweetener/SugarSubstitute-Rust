// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Atomically charge queues, delivery credits, byte reservations and class capacity.

use crate::{
    mailbox::Mailbox,
    memory::Release,
    scope::{Cancellation, ScopeState, earliest, lock},
    task::{ErasedJob, QueuedJob, Stop, stop_reason_at},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Condvar, Mutex},
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, CancelControl, Clock, CompletionNotification, CompletionWake, ExecutionId,
    JobClass, JobContext, JobId, JobPhase, JobSpec, MemoryLease, Moment, Scope, ScopeId,
    TerminalKind,
};
use tokio::sync::Notify;

#[derive(Clone, Copy)]
pub(crate) enum RequestKind {
    Async,
    Blocking,
    Cpu,
}

pub(crate) struct Record {
    pub(crate) scope_id: ScopeId,
    pub(crate) spec: JobSpec,
    pub(crate) phase: JobPhase,
    pub(crate) job: Arc<dyn ErasedJob>,
    pub(crate) retiring: bool,
    pub(crate) acknowledged: bool,
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;

pub(crate) enum Retirement {
    Factory(QueuedJob, Stop),
    Outcome(Arc<dyn ErasedJob>),
}
impl Retirement {
    pub(crate) fn accepts(&self, accepts: impl Fn(JobClass) -> bool) -> bool {
        match self {
            Self::Factory(queued, _) => accepts(queued.context.spec().class),
            Self::Outcome(_) => true,
        }
    }
    fn id(&self) -> JobId {
        match self {
            Self::Factory(queued, _) => queued.context.job_id(),
            Self::Outcome(job) => job.spec().id,
        }
    }
}

pub(crate) struct State {
    pub(crate) scopes: HashMap<ScopeId, ScopeState>,
    pub(crate) jobs: HashMap<JobId, Record>,
    pub(crate) queue: VecDeque<QueuedJob>,
    pub(crate) retired: VecDeque<Retirement>,
    pub(crate) bytes: u64,
    pub(crate) stopping: bool,
    pub(crate) notifications_inflight: usize,
    capacity_wait: capacity::CapacityWait,
    capacity_wakes_inflight: usize,
    latest_job: Option<JobId>,
    latest_scope: Option<ScopeId>,
}

impl State {
    pub(crate) fn settled(&self) -> bool {
        self.jobs.is_empty()
            && self.retired.is_empty()
            && self.notifications_inflight == 0
            && self.capacity_wakes_inflight == 0
            && !self.scopes.values().any(|scope| scope.cancel.pending())
    }
}

pub(crate) struct Core {
    pub(crate) id: ExecutionId,
    pub(crate) state: Mutex<State>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) mailbox: Mailbox,
    pub(crate) notify: Notify,
    pub(crate) cpu_changed: Condvar,
}

fn ordinary(class: JobClass) -> bool {
    !matches!(class, JobClass::IoLifecycle | JobClass::InfrastructureLifecycle)
}

impl Core {
    pub(crate) fn new(
        id: ExecutionId,
        clock: Arc<dyn Clock>,
        wake: Arc<dyn CompletionWake>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            state: Mutex::new(State {
                scopes: HashMap::new(),
                jobs: HashMap::new(),
                queue: VecDeque::with_capacity(32),
                retired: VecDeque::with_capacity(48),
                bytes: 0,
                stopping: false,
                notifications_inflight: 0,
                capacity_wait: capacity::CapacityWait::default(),
                capacity_wakes_inflight: 0,
                latest_job: None,
                latest_scope: None,
            }),
            clock,
            mailbox: Mailbox::new(wake),
            notify: Notify::new(),
            cpu_changed: Condvar::new(),
        })
    }

    pub(crate) fn changed(&self) {
        self.notify.notify_one();
        self.cpu_changed.notify_all();
    }

    pub(crate) fn admit<R>(
        self: &Arc<Self>,
        scope: &Scope,
        spec: JobSpec,
        kind: RequestKind,
        build: impl FnOnce(JobContext, Arc<Cancellation>, Moment) -> (QueuedJob, R),
    ) -> Result<R, AdmissionError> {
        let now = self.clock.now();
        let owner_id = scope.owner_id();
        let mut state = lock(&self.state);
        if state.stopping {
            return Err(AdmissionError::ShuttingDown);
        }
        let stored = self.owned_scope(&state, scope, owner_id)?;
        if stored.closed || stored.cancel.is_requested() {
            return Err(AdmissionError::ScopeClosed { scope_id: scope.id() });
        }
        let deadline = earliest(stored.deadline, spec.deadline);
        let parent_cancel = stored.cancel.clone();
        if let Some(deadline) = deadline.filter(|deadline| *deadline <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        let valid_method = matches!(
            (kind, spec.class),
            (RequestKind::Async, JobClass::IoOrdinary | JobClass::IoLifecycle)
                | (
                    RequestKind::Blocking,
                    JobClass::InfrastructureOrdinary | JobClass::InfrastructureLifecycle
                )
                | (RequestKind::Cpu, JobClass::CpuInteractive | JobClass::CpuDecode)
        );
        if !valid_method || !spec.validates_class() {
            return Err(AdmissionError::WrongClass {
                operation: spec.operation,
                class: spec.class,
            });
        }
        if !spec.validates_replacement() {
            return Err(AdmissionError::InvalidReplacement {
                class: spec.class,
                replacement: spec.replacement,
            });
        }
        if state.jobs.contains_key(&spec.id) {
            return Err(AdmissionError::DuplicateJobId { job_id: spec.id });
        }
        if let Some(latest) = state.latest_job.filter(|latest| spec.id <= *latest) {
            return Err(AdmissionError::JobIdentityOrder { latest });
        }
        if let Some(key) = spec.replacement
            && let Some(current_revision) = state
                .jobs
                .values()
                .filter_map(|record| record.spec.replacement)
                .filter(|current| current.surface == key.surface)
                .map(|current| current.revision)
                .max()
                .filter(|current| *current >= key.revision)
        {
            return Err(AdmissionError::StaleReplacement { replacement: key, current_revision });
        }
        let replacement_index = spec.replacement.and_then(|key| {
            state.queue.iter().position(|queued| {
                queued.context.spec().class == JobClass::CpuInteractive
                    && queued
                        .context
                        .spec()
                        .replacement
                        .is_some_and(|pending| pending.surface == key.surface)
            })
        });
        Self::check_capacity(&mut state, spec, replacement_index.is_some())?;
        state.bytes += spec.reservation_bytes;
        if let Some(stored) = state.scopes.get_mut(&scope.id()) {
            stored.bytes += spec.reservation_bytes;
        }
        let memory = MemoryLease::new(
            spec.reservation_bytes,
            Box::new(Release { core: self.clone(), scope_id: scope.id() }),
        );
        let cancel = Cancellation::new(Some(parent_cancel), Arc::downgrade(self));
        let context = JobContext::new(
            self.id,
            scope.id(),
            spec,
            deadline,
            cancel.clone(),
            self.clock.clone(),
            memory,
        );
        let (queued, result) = build(context, cancel, now);
        state.jobs.insert(
            spec.id,
            Record {
                scope_id: scope.id(),
                spec,
                phase: JobPhase::Queued,
                job: queued.job.clone(),
                retiring: false,
                acknowledged: false,
            },
        );
        state.latest_job = Some(spec.id);
        let replaced = replacement_index.and_then(|index| state.queue.remove(index));
        if let Some(replaced) = replaced {
            if let Some(record) = state.jobs.get_mut(&replaced.context.job_id()) {
                record.retiring = true;
            }
            state.retired.push_back(Retirement::Factory(replaced, Stop::Coalesced(spec.id)));
        }
        state.queue.push_back(queued);
        drop(state);
        self.changed();
        Ok(result)
    }

    pub(crate) fn shutdown(&self) {
        let mut state = lock(&self.state);
        state.stopping = true;
        state.capacity_wait = capacity::CapacityWait::default();
        for scope in state.scopes.values_mut() {
            scope.closed = true;
            scope.cancel.request();
        }
        for record in state.jobs.values() {
            if !record.retiring && !matches!(record.phase, JobPhase::Terminal(_)) {
                record.job.cancellation().request();
            }
        }
        drop(state);
        self.changed();
    }

    fn has_capacity(state: &State, class: JobClass) -> bool {
        let running: Vec<_> = state
            .jobs
            .values()
            .filter(|record| record.phase == JobPhase::Running && !record.retiring)
            .map(|record| record.spec.class)
            .collect();
        match class {
            JobClass::IoOrdinary | JobClass::IoLifecycle => {
                running
                    .iter()
                    .filter(|class| matches!(class, JobClass::IoOrdinary | JobClass::IoLifecycle))
                    .count()
                    < 12
                    && (class == JobClass::IoLifecycle
                        || running.iter().filter(|class| **class == JobClass::IoOrdinary).count()
                            < 8)
            }
            JobClass::InfrastructureOrdinary | JobClass::InfrastructureLifecycle => {
                running
                    .iter()
                    .filter(|class| {
                        matches!(
                            class,
                            JobClass::InfrastructureOrdinary | JobClass::InfrastructureLifecycle
                        )
                    })
                    .count()
                    < 2
                    && (class == JobClass::InfrastructureLifecycle
                        || !running.contains(&JobClass::InfrastructureOrdinary))
            }
            JobClass::CpuInteractive | JobClass::CpuDecode => !running.contains(&class),
        }
    }

    pub(crate) fn take(&self, accepts: impl Fn(JobClass) -> bool) -> Option<QueuedJob> {
        let now = self.clock.now();
        let mut state = lock(&self.state);
        let index = state.queue.iter().position(|queued| {
            accepts(queued.context.spec().class)
                && (stop_reason_at(&queued.context, now).is_some()
                    || Self::has_capacity(&state, queued.context.spec().class))
        })?;
        let queued = state.queue.remove(index)?;
        if let Some(record) = state.jobs.get_mut(&queued.context.job_id()) {
            record.phase = JobPhase::Running;
        }
        queued.job.running();
        let wake = self.capacity_ready(&mut state);
        drop(state);
        self.deliver_capacity_wake(wake);
        Some(queued)
    }

    pub(crate) fn take_stopped_cpu(&self, accepts: impl Fn(JobClass) -> bool) -> Option<QueuedJob> {
        let now = self.clock.now();
        let mut state = lock(&self.state);
        let index = state.queue.iter().position(|queued| {
            accepts(queued.context.spec().class) && stop_reason_at(&queued.context, now).is_some()
        })?;
        let queued = state.queue.remove(index)?;
        if let Some(record) = state.jobs.get_mut(&queued.context.job_id()) {
            record.phase = JobPhase::Running;
            record.retiring = true;
        }
        queued.job.running();
        let wake = self.capacity_ready(&mut state);
        drop(state);
        self.deliver_capacity_wake(wake);
        Some(queued)
    }

    pub(crate) fn take_retired_cpu(
        &self,
        accepts: impl Fn(JobClass) -> bool,
    ) -> Option<Retirement> {
        let mut state = lock(&self.state);
        let index = state.retired.iter().position(|retired| retired.accepts(&accepts))?;
        let retired = state.retired.remove(index)?;
        if let Some(record) = state.jobs.get_mut(&retired.id()) {
            record.phase = JobPhase::Running;
            record.job.running();
        }
        Some(retired)
    }

    pub(crate) fn settled(&self) -> bool {
        lock(&self.state).settled()
    }
}

#[path = "admission_scope.rs"]
mod scopes;

#[path = "admission_notification.rs"]
mod notification;

#[path = "admission_terminal.rs"]
mod terminal;

#[path = "admission_capacity.rs"]
mod capacity;
