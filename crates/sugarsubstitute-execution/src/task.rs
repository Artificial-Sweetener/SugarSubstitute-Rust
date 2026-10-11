// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Supervise typed work, actual termination, cancellation and terminal ownership.

use crate::{
    admission::Core,
    scope::{Cancellation, lock},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    CancelControl, JobCompletion, JobContext, JobControl, JobId, JobOutcome, JobPhase, JobSpec,
    JobStatus, Moment, PanicCause, ScopeId,
};

pub(crate) type TaskFuture = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
pub(crate) type TaskFactory = Box<dyn FnOnce(JobContext, Moment) -> TaskFuture + Send>;

pub(crate) enum Work {
    #[cfg(any(windows, test))]
    Native(crate::native::NativeWork),
    Async(TaskFactory),
    Blocking(TaskFactory),
    Cpu(Box<dyn FnOnce(JobContext, Moment) -> Box<dyn crate::cpu::CpuWork> + Send>),
}

#[cfg(test)]
#[path = "task_tests.rs"]
mod tests;

pub(crate) struct QueuedJob {
    pub(crate) context: JobContext,
    pub(crate) job: Arc<dyn ErasedJob>,
    pub(crate) work: Work,
}

#[derive(Clone, Copy)]
pub(crate) enum Stop {
    Cancelled,
    TimedOut,
    Coalesced(JobId),
}

pub(crate) fn stopped_outcome<T, E>(reason: Stop) -> JobOutcome<T, E> {
    match reason {
        Stop::Cancelled => JobOutcome::Cancelled,
        Stop::TimedOut => JobOutcome::TimedOut,
        Stop::Coalesced(replacement) => JobOutcome::Coalesced { replacement },
    }
}

pub(crate) fn stop_queued(queued: QueuedJob, reason: Stop) {
    let QueuedJob { context, job, work } = queued;
    match catch_unwind(AssertUnwindSafe(|| drop(work))) {
        Ok(()) => job.stop(&context, None, reason),
        Err(payload) => job.panicked(&context, context.now(), panic_cause(payload)),
    }
}

pub(crate) fn stop_reason(context: &JobContext) -> Option<Stop> {
    stop_reason_at(context, context.now())
}

pub(crate) fn stop_reason_at(context: &JobContext, now: Moment) -> Option<Stop> {
    if context.deadline().is_some_and(|deadline| now >= deadline) {
        Some(Stop::TimedOut)
    } else if context.cancellation_requested() {
        Some(Stop::Cancelled)
    } else {
        None
    }
}

pub(crate) trait ErasedJob: Send + Sync {
    fn spec(&self) -> JobSpec;
    fn cancellation(&self) -> Arc<Cancellation>;
    fn dispose(&self);
    fn abandon(&self);
    fn status(&self) -> JobStatus;
    fn running(&self);
    fn stop(&self, context: &JobContext, started: Option<Moment>, reason: Stop);
    fn cancel(&self);
    fn panicked(&self, context: &JobContext, started: Moment, cause: PanicCause);
}

struct Cell<T, E> {
    phase: JobPhase,
    completion: Option<JobCompletion<T, E>>,
    abandoned: bool,
}

pub(crate) struct Job<T, E> {
    pub(crate) scope_id: ScopeId,
    pub(crate) spec: JobSpec,
    admitted: Moment,
    pub(crate) cancel: Arc<Cancellation>,
    core: Weak<Core>,
    cell: Mutex<Cell<T, E>>,
}

impl<T: Send + 'static, E: Send + 'static> Job<T, E> {
    pub(crate) fn new(
        scope_id: ScopeId,
        spec: JobSpec,
        admitted: Moment,
        cancel: Arc<Cancellation>,
        core: Weak<Core>,
    ) -> Arc<Self> {
        Arc::new(Self {
            scope_id,
            spec,
            admitted,
            cancel,
            core,
            cell: Mutex::new(Cell { phase: JobPhase::Queued, completion: None, abandoned: false }),
        })
    }

    pub(crate) fn finish(
        &self,
        context: &JobContext,
        started: Option<Moment>,
        outcome: JobOutcome<T, E>,
    ) {
        let ended = context.now();
        self.finish_at(context, started, ended, outcome);
    }

    /// Publish only after actual stop, using a previously observed owner clock sample.
    pub(crate) fn finish_at(
        &self,
        context: &JobContext,
        started: Option<Moment>,
        ended: Moment,
        outcome: JobOutcome<T, E>,
    ) {
        let start = started.unwrap_or(ended);
        let duration = |later: Moment, earlier: Moment| -> Duration {
            match later.checked_duration_since(earlier) {
                Ok(duration) => duration,
                Err(error) => {
                    tracing::error!(operation = ?self.spec.operation, job_id = self.spec.id.get(), cause = %error, "Injected execution clock regressed");
                    Duration::ZERO
                }
            }
        };
        let kind = outcome.kind();
        let replacement = match &outcome {
            JobOutcome::Coalesced { replacement } => Some(*replacement),
            _ => None,
        };
        let completion = JobCompletion {
            scope_id: self.scope_id,
            spec: self.spec,
            queue_time: duration(start, self.admitted),
            execution_time: duration(ended, start),
            outcome,
            memory: context.memory().clone(),
        };
        if let Some(core) = self.core.upgrade() {
            let wake = core.terminal(self.spec.id, kind, replacement, || {
                let mut cell = lock(&self.cell);
                cell.phase = JobPhase::Terminal(kind);
                cell.completion = Some(completion);
                !cell.abandoned
            });
            if wake {
                core.mailbox.deliver_wake();
            }
            core.changed();
        }
        tracing::debug!(operation = ?self.spec.operation, execution_id = context.owner_id().get(), scope_id = self.scope_id.get(), job_id = self.spec.id.get(), terminal = ?kind, "Owned work stopped");
    }
}

impl<T: Send + 'static, E: Send + 'static> ErasedJob for Job<T, E> {
    fn spec(&self) -> JobSpec {
        self.spec
    }
    fn cancellation(&self) -> Arc<Cancellation> {
        self.cancel.clone()
    }
    fn dispose(&self) {
        let completion = lock(&self.cell).completion.take();
        let failure = catch_unwind(AssertUnwindSafe(|| drop(completion))).err().map(panic_cause);
        if let Some(core) = self.core.upgrade() {
            if let Some(cause) = failure {
                core.mailbox.record_failure(
                    sugarsubstitute_contracts::execution::ExecutionError::new(
                        self.spec.operation,
                        Box::new(std::io::Error::other(cause.as_str().to_owned())),
                    ),
                );
                core.mailbox.deliver_wake();
            }
            lock(&self.cell).phase =
                JobPhase::Terminal(sugarsubstitute_contracts::execution::TerminalKind::Cancelled);
            core.acknowledge(self.spec.id);
        }
    }
    fn abandon(&self) {
        lock(&self.cell).abandoned = true;
        if let Some(core) = self.core.upgrade() {
            core.cancel_job(self.spec.id);
            core.dispose_terminal(self.spec.id);
            core.changed();
        }
    }

    fn status(&self) -> JobStatus {
        JobStatus {
            phase: lock(&self.cell).phase,
            cancellation_requested: self.cancel.is_requested(),
        }
    }
    fn running(&self) {
        lock(&self.cell).phase = JobPhase::Running;
    }
    fn stop(&self, context: &JobContext, started: Option<Moment>, reason: Stop) {
        let outcome = stopped_outcome(reason);
        self.finish(context, started, outcome);
    }
    fn cancel(&self) {
        if let Some(core) = self.core.upgrade() {
            core.cancel_job(self.spec.id);
        }
    }
    fn panicked(&self, context: &JobContext, started: Moment, cause: PanicCause) {
        self.finish(context, Some(started), JobOutcome::Panicked(cause));
    }
}

pub(crate) struct JobHandle<T, E>(pub(crate) Arc<Job<T, E>>);

impl<T: Send + 'static, E: Send + 'static> JobControl<T, E> for JobHandle<T, E> {
    fn cancel(&self) {
        ErasedJob::cancel(self.0.as_ref());
    }
    fn status(&self) -> JobStatus {
        ErasedJob::status(self.0.as_ref())
    }
    fn try_take_completion(&self) -> Option<JobCompletion<T, E>> {
        let completion = lock(&self.0.cell).completion.take();
        if completion.is_some()
            && let Some(core) = self.0.core.upgrade()
        {
            core.acknowledge(self.0.spec.id);
        }
        completion
    }
    fn abandon(&self) {
        ErasedJob::abandon(self.0.as_ref());
    }
}

#[path = "task_async.rs"]
mod asynchronous;
pub(crate) use asynchronous::{panic_cause, run_async};

/// Dispose accepted ownership exclusively on an execution-owned worker.
pub(crate) fn retire(retired: crate::admission::Retirement) {
    match retired {
        crate::admission::Retirement::Factory(queued, reason) => stop_queued(queued, reason),
        crate::admission::Retirement::Outcome(job) => job.dispose(),
    }
}
