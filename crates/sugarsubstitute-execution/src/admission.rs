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
    #[cfg(any(windows, test))]
    Native,
}

pub(crate) struct Record {
    pub(crate) deadline: Option<Moment>,
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
    #[cfg(any(windows, test))]
    pub(crate) native_thread: std::sync::OnceLock<std::thread::Thread>,
    pub(crate) id: ExecutionId,
    pub(crate) state: Mutex<State>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) mailbox: Mailbox,
    pub(crate) notify: Notify,
    pub(crate) cpu_changed: Condvar,
}

fn ordinary(class: JobClass) -> bool {
    !matches!(
        class,
        JobClass::IoLifecycle | JobClass::InfrastructureLifecycle | JobClass::NativeObservation
    )
}

impl Core {
    pub(crate) fn new(
        id: ExecutionId,
        clock: Arc<dyn Clock>,
        wake: Arc<dyn CompletionWake>,
    ) -> Arc<Self> {
        Arc::new(Self {
            #[cfg(any(windows, test))]
            native_thread: std::sync::OnceLock::new(),
            id,
            state: Mutex::new(State {
                scopes: HashMap::new(),
                jobs: HashMap::new(),
                queue: VecDeque::with_capacity(416),
                retired: VecDeque::with_capacity(560),
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
        #[cfg(any(windows, test))]
        if let Some(thread) = self.native_thread.get() {
            thread.unpark();
        }
        self.notify.notify_one();
        self.cpu_changed.notify_all();
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
            JobClass::NativeObservation => {
                running.iter().filter(|class| **class == JobClass::NativeObservation).count() < 384
            }
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

#[cfg(any(windows, test))]
#[path = "admission_native.rs"]
mod native;

#[path = "admission_submit.rs"]
mod submission;
