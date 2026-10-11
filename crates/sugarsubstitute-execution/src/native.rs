// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own admitted native process observations independently of runtime task factories.

use crate::{
    admission::{Core, RequestKind},
    scope::lock,
    task::{ErasedJob, Job, JobHandle, QueuedJob, Work},
};
use std::{
    error::Error,
    fmt,
    num::NonZeroU64,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionError, JobSpec, OwnedJob, Scope,
};

/// Correlate one native observation with an application-assigned positive identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeCorrelation(NonZeroU64);
impl NativeCorrelation {
    /// Reject zero before acquiring any native resource.
    pub fn new(value: u64) -> Result<Self, ExecutionError> {
        NonZeroU64::new(value).map(Self).ok_or_else(|| {
            crate::runtime::error(std::io::Error::other("Native correlation must be positive"))
        })
    }
    /// Return the stable application correlation, without a scheduling capability.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Observe the identity and exit status of one actually signalled process handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessExit {
    /// Correlation supplied before process creation or binding.
    pub correlation: NativeCorrelation,
    /// Kernel process identity validated on the owned duplicate.
    pub process_id: u32,
    /// Kernel process creation timestamp, in 100ns units since January 1, 1601 UTC.
    pub creation_time_100ns: u64,
    /// Actual exit code, including every valid u32 value.
    pub exit_code: u32,
}

/// Preserve the first native stage failure and its original owned cause.
///
/// Up to eight subsequent cleanup failures remain owned; further actual failures
/// are counted explicitly. `source` always identifies the original cause.
#[derive(Clone, Debug)]
pub struct NativeObservationError {
    stage: &'static str,
    cause: Arc<dyn Error + Send + Sync>,
    cleanup: Vec<Arc<NativeObservationError>>,
    omitted_cleanup: u64,
}
impl NativeObservationError {
    pub(crate) fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { stage, cause: Arc::new(cause), cleanup: Vec::new(), omitted_cleanup: 0 }
    }
    /// Identify the native operation whose original cause is retained by `source`.
    pub const fn stage(&self) -> &'static str {
        self.stage
    }
    pub(crate) fn retain_cleanup(&mut self, cleanup: Self) {
        if self.cleanup.len() < 8 {
            self.cleanup.push(Arc::new(cleanup));
        } else {
            self.omitted_cleanup = self.omitted_cleanup.saturating_add(1);
        }
    }
}
impl fmt::Display for NativeObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Native observation {} failed: {}", self.stage, self.cause)?;
        for cleanup in &self.cleanup {
            write!(f, "; cleanup: {cleanup}")?;
        }
        if self.omitted_cleanup != 0 {
            write!(f, "; {} further cleanup failures omitted", self.omitted_cleanup)?;
        }
        Ok(())
    }
}
impl Error for NativeObservationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

pub(crate) trait ProcessLease: Send {
    fn start(
        &mut self,
        correlation: NativeCorrelation,
        signal: Arc<Signal>,
    ) -> Result<(), NativeObservationError>;
    fn read_exit(&mut self) -> Result<ProcessExit, NativeObservationError>;
    /// Return success only after callbacks quiesce and every native handle actually closes.
    fn retire(&mut self) -> Result<(), NativeObservationError>;
}
pub(crate) struct Signal {
    completed: AtomicBool,
    core: Weak<Core>,
}
impl Signal {
    pub(crate) fn new(core: &Arc<Core>) -> Arc<Self> {
        Arc::new(Self { completed: AtomicBool::new(false), core: Arc::downgrade(core) })
    }
    /// Use atomics and private wake delivery only; native callbacks never acquire Core locks.
    pub(crate) fn complete(&self) {
        self.completed.store(true, Ordering::Release);
        if let Some(core) = self.core.upgrade() {
            core.changed();
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed.load(Ordering::Acquire)
    }
}

enum BindingState {
    Pending,
    Bound(Box<dyn ProcessLease>),
    Abandoned,
    Consumed,
}
pub(crate) struct Binding {
    state: Mutex<BindingState>,
}
impl Binding {
    fn new() -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(BindingState::Pending) })
    }
    pub(crate) fn ready(&self) -> bool {
        matches!(*lock(&self.state), BindingState::Bound(_) | BindingState::Abandoned)
    }
    pub(crate) fn take(&self) -> Option<Box<dyn ProcessLease>> {
        match std::mem::replace(&mut *lock(&self.state), BindingState::Consumed) {
            BindingState::Bound(lease) => Some(lease),
            _ => None,
        }
    }
}

/// Reserve all queue, delivery and memory capacity before creating or binding a process.
///
/// Dropping the permit cancels its admitted job and abandons result delivery;
/// execution-owned cleanup retains the real lifetime. Binding or explicit unbound
/// cancellation consumes it once and preserves typed terminal observation.
/// Cancellation retains an unconsumed permit until binding or drop. Consume or
/// drop permits before the owner's final blocking teardown.
#[must_use = "Bind, cancel_unbound, or drop a process-watch permit before final teardown"]
pub struct ProcessWatchPermit {
    pub(crate) job: Arc<Job<ProcessExit, NativeObservationError>>,
    binding: Arc<Binding>,
    core: Arc<Core>,
    transferred: bool,
}
impl ProcessWatchPermit {
    pub(crate) fn cancel_input(mut self) -> OwnedJob<ProcessExit, NativeObservationError> {
        *lock(&self.binding.state) = BindingState::Abandoned;
        self.job.cancel();
        self.transferred = true;
        self.core.changed();
        OwnedJob::new(self.job.scope_id, self.job.spec, Box::new(JobHandle(self.job.clone())))
    }

    pub(crate) fn bind_input(
        mut self,
        input: Box<dyn ProcessLease>,
    ) -> OwnedJob<ProcessExit, NativeObservationError> {
        *lock(&self.binding.state) = BindingState::Bound(input);
        self.core.changed();
        self.transferred = true;
        OwnedJob::new(self.job.scope_id, self.job.spec, Box::new(JobHandle(self.job.clone())))
    }
}
impl Drop for ProcessWatchPermit {
    fn drop(&mut self) {
        if !self.transferred {
            *lock(&self.binding.state) = BindingState::Abandoned;
            self.job.abandon();
            self.core.changed();
        }
    }
}
pub(crate) struct NativeWork {
    pub(crate) binding: Arc<Binding>,
    pub(crate) job: Arc<Job<ProcessExit, NativeObservationError>>,
    pub(crate) correlation: NativeCorrelation,
}

pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    correlation: NativeCorrelation,
) -> Result<ProcessWatchPermit, AdmissionError> {
    core.admit(scope, spec, RequestKind::Native, |context, cancel, admitted| {
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let binding = Binding::new();
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(NativeWork {
                binding: binding.clone(),
                job: job.clone(),
                correlation,
            }),
        };
        let permit = ProcessWatchPermit { job, binding, core: core.clone(), transferred: false };
        (queued, permit)
    })
}

#[path = "native_worker.rs"]
mod worker;
pub(crate) use worker::NativeWorker;

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
