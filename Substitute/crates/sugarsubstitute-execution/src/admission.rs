// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Atomically charge queues, delivery credits, byte reservations and class capacity.

use crate::{
    mailbox::Mailbox,
    memory::Release,
    scope::{Cancellation, ScopeState, earliest, lock},
    task::{ErasedJob, QueuedJob, Stop},
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
/// Identify construction without extending source ownership beyond synchronous admission.
/// Captured and symbol inputs retain genuine original bindings outside Core while this request borrows them;
/// validation and the factory share those original allocations without a new control.
pub(crate) enum RequestKind<'a> {
    #[cfg(all(feature = "k1-native-qualification", any(windows, test)))]
    RegisteredDesktopFault {
        custody: &'a crate::desktop_capture::metadata::DesktopCaptureCustodyRef,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
    },
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
    ReportExportCleanup {
        main_scope: sugarsubstitute_contracts::execution::ScopeSpec,
        main_spec: JobSpec,
    },
    ReportExportMain {
        cleanup: JobId,
    },
    SymbolNative {
        original: &'a JobContext,
        handles: Arc<sugarsubstitute_contracts::execution::NativeHandleClaim>,
    },
    #[cfg(any(windows, test))]
    Native,
    #[cfg(any(windows, test))]
    RecoveryPipe(&'a crate::RegisteredRecoveryGrant),
    #[cfg(any(windows, test))]
    ProcessCreation(&'a crate::process_creation::source::Source),
    #[cfg(any(windows, test))]
    BackendCustody {
        source: &'a crate::lifecycle_pipe::SharedRef,
        original: &'a crate::lifecycle_pipe::Owner,
    },
    #[cfg(any(windows, test))]
    CapturedLifecycle(&'a crate::lifecycle_pipe::captured_admission::AdmissionBinding),
    #[cfg(any(windows, test))]
    CaptureAttach(sugarsubstitute_contracts::crash::CaptureTarget),
    #[cfg(any(windows, test))]
    DesktopCapture(sugarsubstitute_contracts::crash::CaptureTarget),
}

pub(crate) struct Record {
    #[cfg(any(windows, test))]
    pub(crate) process_creation: bool,
    #[cfg(any(windows, test))]
    pub(crate) custody_deadlines: Option<Box<crate::backend_custody::deadlines::Deadlines>>,
    pub(crate) memory: Option<sugarsubstitute_contracts::execution::MemoryLeaseObservation>,
    pub(crate) entry_setup: Option<sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget>,
    pub(crate) preparation_issued: bool,
    pub(crate) preparation: Option<Arc<crate::entry_preparation::slot::PreparationSlot>>,
    pub(crate) registered_startup_issued: bool,
    pub(crate) capture_bundle_issued: bool,
    #[cfg(any(windows, test))]
    pub(crate) capture_event_issued: bool,
    #[cfg(any(windows, test))]
    pub(crate) capture_event_identity: Option<usize>,
    pub(crate) symbol_original: Option<JobId>,
    pub(crate) report_pair: Option<crate::report_export::pair::Association>,
    pub(crate) report_export: bool,
    pub(crate) report_export_issued: bool,
    pub(crate) report_cleanup: Option<crate::report_export::cleanup::Binding>,
    pub(crate) symbol_cleanup: Option<crate::symbol_cleanup::Binding>,
    #[cfg(any(windows, test))]
    pub(crate) native_custody: Option<Arc<crate::capture_attach::control::Custody>>,
    #[cfg(any(windows, test))]
    pub(crate) desktop_capture_custody:
        Option<crate::desktop_capture::metadata::DesktopCaptureCustodyRef>,
    #[cfg(any(windows, test))]
    pub(crate) process_capture:
        Option<crate::capture_bundle::desktop_transfer::ProcessCaptureAssociation>,
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
    /// Retain child metadata outside any cleanup callback borrowed by Core.
    #[cfg(any(windows, test))]
    pub(crate) fn native_custody_loan(
        &self,
    ) -> Option<crate::desktop_capture::metadata::NativeCustodyLoan> {
        self.native_custody.as_ref().map(|custody| {
            crate::desktop_capture::metadata::NativeCustodyLoan::new(
                custody.clone(),
                self.desktop_capture_custody.as_ref(),
            )
        })
    }
    pub(crate) fn desktop_capture_child(&self) -> bool {
        #[cfg(any(windows, test))]
        {
            self.desktop_capture_custody.is_some()
        }
        #[cfg(not(any(windows, test)))]
        {
            false
        }
    }
    pub(crate) fn process_lifetime_only(&self) -> bool {
        #[cfg(any(windows, test))]
        {
            self.process_capture.as_ref().is_some_and(|capture| capture.capsule_identity != 0)
                && self.acknowledged
                && self.phase == JobPhase::Terminal(TerminalKind::Success)
                && !self.retiring
                && !self.job.cancellation().pending()
                && self.native_custody.is_none()
                && self.desktop_capture_custody.is_none()
        }
        #[cfg(not(any(windows, test)))]
        {
            false
        }
    }
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
    pub(crate) report_resources: HashMap<JobId, crate::report_export::resources::ResourceRecord>,
    pub(crate) symbol_resources: HashMap<JobId, crate::native_resources::ResourceRecord>,
    pub(crate) scopes: HashMap<ScopeId, ScopeState>,
    pub(crate) jobs: HashMap<JobId, Record>,
    pub(crate) queue: VecDeque<QueuedJob>,
    pub(crate) retired: VecDeque<Retirement>,
    pub(crate) bytes: u64,
    pub(crate) byte_limit: u64,
    pub(crate) stopping: bool,
    pub(crate) notifications_inflight: usize,
    capacity_wait: capacity::CapacityWait,
    capacity_wakes_inflight: usize,
    latest_job: Option<JobId>,
    latest_scope: Option<ScopeId>,
}

impl State {
    pub(crate) fn settled(&self) -> bool {
        self.jobs.values().all(Record::process_lifetime_only)
            && self.retired.is_empty()
            && self.notifications_inflight == 0
            && self.capacity_wakes_inflight == 0
            && !self.scopes.values().any(|scope| scope.cancel.pending())
    }

    /// A selected capacity wake is still owed to the completion wake, and shutdown has not
    /// suppressed it. The release it announces is already observable under this lock.
    pub(crate) fn capacity_wake_pending(&self) -> bool {
        !self.stopping && self.capacity_wakes_inflight > 0
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
    pub(crate) workers: crate::worker_exit::WorkerCensus,
    /// Bounded helper capacity lent to CPU jobs for borrowed fork-join work.
    pub(crate) helpers: Arc<crate::fork_join::Helpers>,
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
        byte_limit: u64,
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
                report_resources: HashMap::new(),
                symbol_resources: HashMap::new(),
                scopes: HashMap::new(),
                jobs: HashMap::new(),
                queue: VecDeque::with_capacity(416),
                retired: VecDeque::with_capacity(560),
                bytes: 0,
                byte_limit,
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
            workers: crate::worker_exit::WorkerCensus::default(),
            helpers: Arc::new(crate::fork_join::Helpers::new(
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
            )),
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
        let capture =
            state.jobs.values().filter_map(Record::native_custody_loan).collect::<Vec<_>>();
        drop(state);
        #[cfg(any(windows, test))]
        for custody in capture {
            crate::capture_attach::control::request(
                &custody.custody,
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
            .map(|record| {
                (record.report_pair.map_or(record.spec.id, |pair| pair.root), record.spec.class)
            })
            .collect::<std::collections::HashMap<_, _>>()
            .into_values()
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
            queued.factory_work()
                && accepts(queued.context.spec().class)
                && (queued.stop_reason(now).is_some()
                    || (!matches!(queued.work, crate::task::Work::BlockingHeld(_))
                        && Self::has_capacity(&state, queued.context.spec().class)))
        })?;
        let queued = state.queue.remove(index)?;
        if let Some(record) = state.jobs.get_mut(&queued.context.job_id()) {
            record.phase = JobPhase::Running;
        }
        queued.job.running();
        if let crate::task::Work::ReportExportPair(pair) = &queued.work {
            if let Some(record) = state.jobs.get_mut(&pair.context.job_id()) {
                record.phase = JobPhase::Running;
            }
            pair.job.running();
        }
        let wake = self.capacity_ready(&mut state);
        drop(state);
        self.deliver_capacity_wake(wake);
        Some(queued)
    }

    pub(crate) fn take_stopped_cpu(&self, accepts: impl Fn(JobClass) -> bool) -> Option<QueuedJob> {
        let now = self.clock.retirement_now();
        let mut state = lock(&self.state);
        let index = state.queue.iter().position(|queued| {
            accepts(queued.context.spec().class) && queued.stop_reason(now).is_some()
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
                Some(crate::task::FiniteRetirement::Factory(Box::new(queued), reason))
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

#[path = "admission_record.rs"]
mod record;
#[path = "report_export/pair_admission.rs"]
mod report_pair;
