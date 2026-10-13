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

#[derive(Clone)]
pub(crate) enum RequestKind {
    Async,
    Blocking,
    Cpu,
    EntryPreparation(sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget),
    SymbolCleanup {
        main_scope: sugarsubstitute_contracts::execution::ScopeSpec,
        main_spec: JobSpec,
    },
    SymbolHelper {
        cleanup: JobId,
    },
    SymbolNative {
        original: JobContext,
        handles: Arc<sugarsubstitute_contracts::execution::NativeHandleClaim>,
    },
    #[cfg(any(windows, test))]
    Native,
    #[cfg(any(windows, test))]
    CaptureAttach(sugarsubstitute_contracts::lifecycle::RunKey),
}

pub(crate) struct Record {
    pub(crate) memory: Option<sugarsubstitute_contracts::execution::MemoryLeaseObservation>,
    pub(crate) entry_setup: Option<sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget>,
    pub(crate) preparation_issued: bool,
    pub(crate) preparation: Option<Arc<crate::entry_preparation::slot::PreparationSlot>>,
    pub(crate) symbol_original: Option<JobId>,
    pub(crate) symbol_cleanup: Option<crate::symbol_cleanup::Binding>,
    #[cfg(any(windows, test))]
    pub(crate) native_custody: Option<Arc<crate::capture_attach::control::Custody>>,
    #[cfg(any(windows, test))]
    pub(crate) native_deadlines: Option<crate::native_deadlines::Deadlines>,
    pub(crate) deadline: Option<Moment>,
    pub(crate) scope_id: ScopeId,
    pub(crate) spec: JobSpec,
    pub(crate) phase: JobPhase,
    pub(crate) job: Arc<dyn ErasedJob>,
    pub(crate) retiring: bool,
    pub(crate) acknowledged: bool,
    pub(crate) terminal_published: bool,
}

impl Record {
    pub(crate) fn held_native(&self) -> bool {
        if self.preparation.as_ref().is_some_and(|slot| slot.held()) {
            return true;
        }
        #[cfg(any(windows, test))]
        {
            self.native_custody.as_ref().is_some_and(|custody| custody.held())
        }
        #[cfg(not(any(windows, test)))]
        {
            false
        }
    }
    fn active(&self) -> bool {
        if self.preparation.as_ref().is_some_and(|slot| slot.held()) {
            return true;
        }
        if self.phase == JobPhase::Running && !self.retiring {
            return true;
        }
        #[cfg(any(windows, test))]
        {
            self.native_custody.as_ref().is_some_and(|custody| custody.active())
        }
        #[cfg(not(any(windows, test)))]
        {
            false
        }
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;

pub(crate) enum Retirement {
    #[cfg(any(windows, test))]
    Environment(crate::entry_preparation::retirement::EntryPreparationRetirement),
    Factory(QueuedJob, Stop),
    Outcome(Arc<dyn ErasedJob>),
}
impl Retirement {
    pub(crate) fn accepts(&self, accepts: impl Fn(JobClass) -> bool) -> bool {
        match self {
            Self::Factory(queued, _) => accepts(queued.context.spec().class),
            Self::Outcome(_) => true,
            #[cfg(any(windows, test))]
            Self::Environment(_) => false,
        }
    }
    fn id(&self) -> JobId {
        match self {
            Self::Factory(queued, _) => queued.context.job_id(),
            Self::Outcome(job) => job.spec().id,
            #[cfg(any(windows, test))]
            Self::Environment(retired) => retired.job_id(),
        }
    }
}

pub(crate) struct State {
    pub(crate) symbol_resources: HashMap<JobId, crate::native_resources::ResourceRecord>,
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
    #[cfg(any(windows, test))]
    pub(crate) native_identity_unrestored: std::sync::atomic::AtomicBool,
    pub(crate) id: ExecutionId,
    pub(crate) state: Mutex<State>,
    pub(crate) clock: Arc<crate::clock_retirement::RetirementClock>,
    pub(crate) mailbox: Mailbox,
    pub(crate) notify: Arc<Notify>,
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
        let mailbox = Mailbox::new(wake);
        let notify = mailbox.notify.clone();
        Arc::new_cyclic(|owner| Self {
            #[cfg(any(windows, test))]
            native_thread: std::sync::OnceLock::new(),
            #[cfg(any(windows, test))]
            native_identity_unrestored: std::sync::atomic::AtomicBool::new(false),
            id,
            state: Mutex::new(State {
                symbol_resources: HashMap::new(),
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
            clock: Arc::new(crate::clock_retirement::RetirementClock::new(clock, owner.clone())),
            mailbox,
            notify,
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
        #[cfg(any(windows, test))]
        let capture = state
            .jobs
            .values()
            .filter_map(|record| record.native_custody.clone())
            .collect::<Vec<_>>();
        drop(state);
        #[cfg(any(windows, test))]
        for custody in capture {
            crate::capture_attach::control::request(
                &custody,
                crate::capture_attach::control::CleanupTrigger::OwnerShutdown,
            );
        }
        self.changed();
    }

    fn has_capacity(state: &State, class: JobClass) -> bool {
        let running: Vec<_> = state
            .jobs
            .values()
            .filter(|record| record.active())
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
        let now = self.clock.retirement_now();
        let mut state = lock(&self.state);
        let index = state.queue.iter().position(|queued| {
            accepts(queued.context.spec().class)
                && (stop_reason_at(&queued.context, now).is_some()
                    || (!matches!(queued.work, crate::task::Work::BlockingHeld(_))
                        && Self::has_capacity(&state, queued.context.spec().class)))
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
        let now = self.clock.retirement_now();
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
    ) -> Option<crate::task::FiniteRetirement> {
        let mut state = lock(&self.state);
        let index = state.retired.iter().position(|retired| retired.accepts(&accepts))?;
        let retired = state.retired.remove(index)?;
        if let Some(record) = state.jobs.get_mut(&retired.id()) {
            record.phase = JobPhase::Running;
            record.job.running();
        }
        match retired {
            Retirement::Factory(queued, reason) => {
                Some(crate::task::FiniteRetirement::Factory(queued, reason))
            }
            Retirement::Outcome(job) => Some(crate::task::FiniteRetirement::Outcome(job)),
            #[cfg(any(windows, test))]
            Retirement::Environment(retired) => {
                state.retired.push_front(Retirement::Environment(retired));
                None
            }
        }
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
