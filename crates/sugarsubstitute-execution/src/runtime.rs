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
pub use policy::{ExecutionLimits, ShutdownProgress, WorkerStatus};

/// Own runtime and CPU joins through teardown after the native event loop exits.
///
/// Request/poll shutdown while UI remains live. `finish_shutdown` requires actual
/// settlement and must run after native teardown. Dropping without this transaction
/// cancels and joins synchronously, and can block until finite work returns.
pub struct ExecutionOwner {
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
    core: Arc<Core>,
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
        let core = Core::new(id, clock, wake);
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .thread_name("sugarsubstitute-async")
            .enable_time()
            .build()
            .map_err(error)?;
        let cpu = CpuPool::start(core.clone(), cpu_workers)?;
        let supervisor = runtime.spawn(supervise(core.clone()));
        Ok(Self {
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
        let now = self.core.clock.now();
        let state = lock(&self.core.state);
        let mut unresolved_job_ids: Vec<_> = state.jobs.keys().copied().collect();
        unresolved_job_ids.sort_unstable();
        let queued = state.jobs.values().filter(|record| record.phase == JobPhase::Queued).count();
        let running =
            state.jobs.values().filter(|record| record.phase == JobPhase::Running).count();
        let workers = WorkerStatus {
            async_finished: self.supervisor.as_ref().is_none_or(JoinHandle::is_finished),
            cpu_finished: self.cpu.as_ref().is_none_or(CpuPool::is_finished),
        };
        let settled =
            state.stopping && state.settled() && workers.async_finished && workers.cpu_finished;
        Ok(ShutdownProgress {
            settled,
            deadline_expired: !settled
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
        let mut failures = Vec::with_capacity(2);
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

impl sugarsubstitute_contracts::execution::ExecutionPort for ExecutionClient {
    fn create_scope(
        &self,
        spec: ScopeSpec,
        parent: Option<&Scope>,
    ) -> Result<Scope, AdmissionError> {
        self.core.create_scope(spec, parent)
    }
    fn submit_async<T, E, F, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        factory: M,
    ) -> Result<OwnedJob<T, E>, AdmissionError>
    where
        T: Send + 'static,
        E: Send + 'static,
        F: Future<Output = Result<T, E>> + Send + 'static,
        M: FnOnce(sugarsubstitute_contracts::execution::JobContext) -> F + Send + 'static,
    {
        self.submit(scope, spec, RequestKind::Async, |job| {
            Work::Async(Box::new(move |context, started| {
                Box::pin(task::run_async(job, context, started, factory))
            }))
        })
    }
    fn submit_blocking<T, E, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        factory: M,
    ) -> Result<OwnedJob<T, E>, AdmissionError>
    where
        T: Send + 'static,
        E: Send + 'static,
        M: FnOnce(sugarsubstitute_contracts::execution::JobContext) -> Result<T, E>
            + Send
            + 'static,
    {
        self.submit(scope, spec, RequestKind::Blocking, |job| {
            Work::Blocking(Box::new(move |context, started| {
                Box::pin(crate::blocking::run(job, context, started, factory))
            }))
        })
    }
    fn submit_cpu<T, E, C, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        factory: M,
    ) -> Result<OwnedJob<T, E>, AdmissionError>
    where
        T: Send + 'static,
        E: Send + 'static,
        C: CpuTask<T, E>,
        M: FnOnce(sugarsubstitute_contracts::execution::JobContext) -> C + Send + 'static,
    {
        self.submit(scope, spec, RequestKind::Cpu, |job| {
            Work::Cpu(Box::new(move |context, started| {
                crate::cpu::work(job, context, started, factory)
            }))
        })
    }
    fn drain_completions(&self, maximum: NonZeroUsize) -> Result<CompletionBatch, ExecutionError> {
        self.core.mailbox.drain(maximum)
    }
}

impl ExecutionClient {
    #[cfg(test)]
    pub(crate) fn core_for_test(&self) -> Arc<Core> {
        self.core.clone()
    }
    fn submit<T: Send + 'static, E: Send + 'static>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        kind: RequestKind,
        work: impl FnOnce(Arc<Job<T, E>>) -> Work,
    ) -> Result<OwnedJob<T, E>, AdmissionError> {
        self.core.admit(scope, spec, kind, |context, cancel, admitted| {
            let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(&self.core));
            let queued = QueuedJob { context, job: job.clone(), work: work(job.clone()) };
            (queued, OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))))
        })
    }
}

#[path = "runtime_supervision.rs"]
mod supervision;
use supervision::supervise;

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
