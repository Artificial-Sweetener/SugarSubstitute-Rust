// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose the sole private scheduler and retain every worker through actual shutdown.

use crate::{
    admission::{Core, RequestKind},
    cpu::CpuPool,
    scope::lock,
    task::{self, Job, JobHandle, QueuedJob, Work},
};
use std::{
    future::Future,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, Clock, CompletionBatch, CompletionWake, CpuTask, ExecutionError, ExecutionId,
    JobPhase, JobSpec, Moment, Operation, OwnedJob, Scope, ScopeSpec,
};
use tokio::{
    runtime::{Builder, Runtime},
    task::JoinHandle,
};

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

#[path = "runtime_policy.rs"]
mod policy;
pub use policy::{
    EntryEnvironmentCustody, EntryEnvironmentExtent, ExecutionLimits, NativeOutputCustody,
    ProcessCaptureCustody, ShutdownProgress, WorkerStatus,
};

/// Own runtime and CPU joins through teardown after the native event loop exits.
///
/// Request/poll shutdown while UI remains live. `finish_shutdown` requires actual
/// settlement and must run after native teardown. Dropping without this transaction
/// cancels and joins synchronously, and can block until finite work returns.
/// Consume or drop native process-watch permits before this final teardown;
/// an unconsumed permit remains unresolved because it can still bind a handle.
pub struct ExecutionOwner {
    #[cfg(any(windows, test))]
    native: Option<crate::native::NativeWorker>,
    core: Arc<Core>,
    runtime: Option<Runtime>,
    supervisor: Option<JoinHandle<()>>,
    cpu: Option<CpuPool>,
    shutdown_deadline: Option<Moment>,
    limits: ExecutionLimits,
}

/// Clone an application submission endpoint without exposing scheduler handles.
#[derive(Clone)]
pub struct ExecutionClient {
    pub(crate) core: Arc<Core>,
}

impl ExecutionClient {
    /// Sample the existing owner's monotonic clock outside internal locks.
    ///
    /// The result is nanoseconds in this executor's injected epoch, useful for
    /// validating original use-case budgets before admission. It exposes no
    /// scheduler, timer or clock-provider capability.
    ///
    /// # Errors
    /// A supplied clock panic is an unexpected owner failure with its bounded
    /// original panic cause. It permanently closes admission and requests actual
    /// cancellation. Later calls retain that cause; cached retirement metadata
    /// cannot establish fresh elapsed time, deadline expiry or timer progress.
    pub fn now(&self) -> Result<Moment, ExecutionError> {
        match self.core.clock.metadata() {
            Ok(moment) => Ok(moment),
            Err(cause) => {
                self.core.shutdown();
                Err(cause)
            }
        }
    }
}

pub(crate) fn error(cause: impl std::error::Error + Send + Sync + 'static) -> ExecutionError {
    ExecutionError::new(Operation::Infrastructure, Box::new(cause))
}

#[path = "runtime_errors.rs"]
mod errors;
pub(crate) use errors::aggregate_errors;

impl ExecutionOwner {
    /// Construct fixed workers using an injected monotonic clock and native wake.
    ///
    /// Returns original runtime/thread creation causes; partial construction joins
    /// already-created workers before returning an error.
    pub fn new(
        clock: Arc<dyn Clock>,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<Self, ExecutionError> {
        let logical = std::thread::available_parallelism().map_err(error)?.get();
        Self::build(clock, wake, if logical == 1 { 1 } else { 2 })
    }

    fn build(
        clock: Arc<dyn Clock>,
        wake: Arc<dyn CompletionWake>,
        cpu_workers: usize,
    ) -> Result<Self, ExecutionError> {
        let raw_id = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| error(std::io::Error::other("Execution identity exhausted")))?;
        let id = ExecutionId::new(raw_id).map_err(error)?;
        let core =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Core::new(id, clock, wake)))
                .map_err(|payload| {
                error(std::io::Error::other(task::panic_cause(payload).as_str().to_owned()))
            })?;
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .thread_name("sugarsubstitute-async")
            .enable_time()
            .build()
            .map_err(error)?;
        let cpu = CpuPool::start(core.clone(), cpu_workers)?;
        #[cfg(any(windows, test))]
        let native = match crate::native::NativeWorker::start(core.clone()) {
            Ok(native) => native,
            Err(primary) => {
                core.shutdown();
                let mut failures = vec![primary];
                if let Err(error) = cpu.finish() {
                    failures.push(error);
                }
                return Err(aggregate_errors(failures));
            }
        };
        let supervisor = runtime.spawn(supervise(core.clone()));
        Ok(Self {
            #[cfg(any(windows, test))]
            native: Some(native),
            core,
            runtime: Some(runtime),
            supervisor: Some(supervisor),
            cpu: Some(cpu),
            shutdown_deadline: None,
            limits: ExecutionLimits::fixed(cpu_workers),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_cpu_workers(
        clock: Arc<dyn Clock>,
        wake: Arc<dyn CompletionWake>,
        workers: usize,
    ) -> Result<Self, ExecutionError> {
        if !matches!(workers, 1 | 2) {
            return Err(error(std::io::Error::other("CPU test worker count must be one or two")));
        }
        Self::build(clock, wake, workers)
    }

    /// Obtain an owned typed submission endpoint sharing this execution identity.
    pub fn client(&self) -> ExecutionClient {
        ExecutionClient { core: self.core.clone() }
    }

    /// Observe immutable fixed limits used by this executor.
    pub fn limits(&self) -> ExecutionLimits {
        self.limits
    }

    /// Stop admission and request cancellation without waiting on any worker.
    ///
    /// Repeated requests retain the earliest deadline. Expiry never detaches work.
    pub fn request_shutdown(&mut self, deadline: Moment) {
        self.shutdown_deadline = crate::scope::earliest(self.shutdown_deadline, Some(deadline));
        self.core.shutdown();
    }

    /// Inspect actual job/worker settlement without blocking the UI thread.
    ///
    /// Untransferred results and pending disposal/notification work prevent settlement.
    /// Returns observed native-wake, cancellation-waker or disposal failure causes.
    pub fn poll_shutdown(&self) -> Result<ShutdownProgress, ExecutionError> {
        if let Some(error) = self.core.mailbox.take_failure() {
            return Err(error);
        }
        let now = self.core.clock.retirement_now();
        let state = lock(&self.core.state);
        let mut unresolved_job_ids: Vec<_> = state
            .jobs
            .iter()
            .filter(|(_, record)| !record.process_lifetime_only())
            .map(|(id, _)| *id)
            .collect();
        unresolved_job_ids.sort_unstable();
        let queued = state.jobs.values().filter(|record| record.phase == JobPhase::Queued).count();
        let running =
            state.jobs.values().filter(|record| record.phase == JobPhase::Running).count();
        let workers = WorkerStatus {
            async_finished: self.supervisor.as_ref().is_none_or(JoinHandle::is_finished),
            cpu_finished: self.cpu.as_ref().is_none_or(CpuPool::is_finished),
            native_finished: {
                #[cfg(any(windows, test))]
                {
                    self.native.as_ref().is_none_or(crate::native::NativeWorker::is_finished)
                }
                #[cfg(not(any(windows, test)))]
                {
                    true
                }
            },
        };
        let entry_environment_custody = state
            .jobs
            .values()
            .filter_map(|record| {
                let slot = record.preparation.as_ref()?;
                if !slot.held() {
                    return None;
                }
                let passive = lock(&slot.state);
                Some(EntryEnvironmentCustody {
                    scope_id: slot.scope,
                    job_id: slot.job,
                    extent: EntryEnvironmentExtent::Unmeasured,
                    retirement_uncertain: passive.phase
                        == crate::entry_preparation::slot::PreparationPhase::Uncertain,
                    callback_quiescent: passive.callback_quiescent,
                })
            })
            .collect::<Vec<_>>();
        #[cfg(any(windows, test))]
        let process_capture_custody = state
            .jobs
            .values()
            .filter_map(|record| {
                let capture = record.process_capture.as_ref()?;
                Some(ProcessCaptureCustody {
                    scope_id: record.scope_id,
                    job_id: record.spec.id,
                    full_target: capture.full_target,
                    reservation_bytes: record.spec.reservation_bytes,
                })
            })
            .collect();
        #[cfg(not(any(windows, test)))]
        let process_capture_custody = Vec::new();
        let native_output_custody = crate::native_resources::output_custody(&state);
        let settled = state.stopping
            && native_output_custody.is_empty()
            && entry_environment_custody.is_empty()
            && state.settled()
            && workers.async_finished
            && workers.cpu_finished
            && workers.native_finished;
        Ok(ShutdownProgress {
            process_capture_custody,
            native_output_custody,
            entry_environment_custody,
            settled,
            deadline_expired: !self.core.clock.failed()
                && !settled
                && self.shutdown_deadline.is_some_and(|deadline| now >= deadline),
            unresolved_job_ids,
            queued,
            running,
            workers,
        })
    }

    /// Join actually stopped workers and destroy the private runtime after UI exit.
    ///
    /// Returns an error and keeps ownership when work has not stopped. Worker panic
    /// causes are observed during joins and retained as execution errors.
    pub fn finish_shutdown(&mut self) -> Result<(), ExecutionError> {
        if !self.poll_shutdown()?.settled {
            return Err(error(std::io::Error::other("Execution shutdown remains unsettled")));
        }
        self.join_workers()
    }

    fn join_workers(&mut self) -> Result<(), ExecutionError> {
        let mut failures = Vec::with_capacity(3);
        #[cfg(any(windows, test))]
        if let Some(native) = self.native.take()
            && let Err(cause) = native.finish()
        {
            failures.push(cause);
        }
        if let Some(handle) = self.supervisor.take()
            && let Some(runtime) = &self.runtime
            && let Err(cause) = runtime.block_on(handle)
        {
            failures.push(error(cause));
        }
        if let Some(cpu) = self.cpu.take()
            && let Err(cause) = cpu.finish()
        {
            failures.push(cause);
        }
        drop(self.runtime.take());
        if failures.is_empty() { Ok(()) } else { Err(aggregate_errors(failures)) }
    }
}

impl Drop for ExecutionOwner {
    fn drop(&mut self) {
        self.core.abandon_all();
        self.core.shutdown();
        if let Err(error) = self.join_workers() {
            tracing::error!(operation = ?error.operation(), cause = %error, "Owned execution teardown failed");
        }
    }
}

#[path = "runtime_supervision.rs"]
mod supervision;
use supervision::supervise;

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_clock_metadata_tests.rs"]
mod clock_metadata_tests;

#[path = "runtime_submit.rs"]
mod submission;
