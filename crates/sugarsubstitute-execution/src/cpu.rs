// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own fixed CPU lanes and supervise bounded resumable preparation steps.

use crate::{
    admission::Core,
    scope::lock,
    task::{Job, QueuedJob, Stop, Work, panic_cause, retire, stop_queued, stop_reason},
    worker_exit::{WorkerExit, WorkerKind},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
    thread::{self, JoinHandle},
};
use sugarsubstitute_contracts::execution::{
    CpuStep, CpuTask, ExecutionError, JobClass, JobContext, JobOutcome, Moment, Operation,
    PanicCause,
};

/// Distinguish retained step state from actual terminal work.
pub(crate) enum CpuProgress {
    /// The same task remains owned, with its running admission credit.
    Yield,
    /// Actual task destruction and terminal publication have finished.
    Stopped,
}
/// Erase task payloads only inside the execution-owned worker.
pub(crate) trait CpuWork: Send {
    /// Run one bounded step and observe its cancellation/destruction boundaries.
    fn step(&mut self) -> CpuProgress;
}

/// Retain a single factory and its constructed task through cooperative yields.
/// The factory is taken before invocation; the task is stored only after it
/// returns, so these two owned states cannot coexist.
struct TypedWork<T, E, C, M> {
    job: Arc<Job<T, E>>,
    context: JobContext,
    started: Moment,
    factory: Option<M>,
    task: Option<C>,
}

/// Observe destruction before releasing running credit or publishing a terminal result.
fn dispose<T>(value: T) -> Result<(), PanicCause> {
    catch_unwind(AssertUnwindSafe(|| drop(value))).map_err(panic_cause)
}

impl<T, E, C, M> TypedWork<T, E, C, M>
where
    T: Send + 'static,
    E: Send + 'static,
    C: CpuTask<T, E>,
    M: FnOnce(JobContext) -> C + Send + 'static,
{
    /// Destroy actual task state before reporting success, failure or cancellation.
    fn finish(&mut self, outcome: JobOutcome<T, E>) -> CpuProgress {
        let task_failure = dispose(self.task.take()).err();
        let factory_failure = dispose(self.factory.take()).err();
        // Factory/task ownership is exclusive, so at most one destructor can fail.
        let outcome = match task_failure.or(factory_failure) {
            Some(cause) => {
                let cause = match dispose(outcome) {
                    Ok(()) => cause,
                    Err(result_cause) => PanicCause::new(format!(
                        "{}; result destruction: {}",
                        cause.as_str(),
                        result_cause.as_str()
                    )),
                };
                JobOutcome::Panicked(cause)
            }
            None => outcome,
        };
        self.job.finish(&self.context, Some(self.started), outcome);
        CpuProgress::Stopped
    }

    /// Drop a discarded step result under supervision when a stop request won during work.
    fn stopped(&mut self, reason: Stop, result: Option<Result<CpuStep<T>, E>>) -> CpuProgress {
        let outcome = match dispose(result) {
            Err(cause) => JobOutcome::Panicked(cause),
            Ok(()) => match reason {
                Stop::TimedOut => JobOutcome::TimedOut,
                Stop::Cancelled => JobOutcome::Cancelled,
                Stop::Coalesced(replacement) => JobOutcome::Coalesced { replacement },
            },
        };
        self.finish(outcome)
    }
}

impl<T, E, C, M> CpuWork for TypedWork<T, E, C, M>
where
    T: Send + 'static,
    E: Send + 'static,
    C: CpuTask<T, E>,
    M: FnOnce(JobContext) -> C + Send + 'static,
{
    fn step(&mut self) -> CpuProgress {
        if let Some(reason) = stop_reason(&self.context) {
            return self.stopped(reason, None);
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            if let Some(factory) = self.factory.take() {
                self.task = Some(factory(self.context.clone()));
            }
            self.task.as_mut().map(|task| task.step(&self.context))
        }));
        match result {
            Err(payload) => self.finish(JobOutcome::Panicked(panic_cause(payload))),
            Ok(Some(result)) => {
                if let Some(reason) = stop_reason(&self.context) {
                    return self.stopped(reason, Some(result));
                }
                match result {
                    Ok(CpuStep::Yield) => CpuProgress::Yield,
                    Ok(CpuStep::Complete(value)) => self.finish(JobOutcome::Success(value)),
                    Err(error) => self.finish(JobOutcome::Failure(error)),
                }
            }
            Ok(None) => {
                self.finish(JobOutcome::Panicked(PanicCause::new("Missing owned CPU task".into())))
            }
        }
    }
}

/// Construct private supervision after admission without invoking the application factory.
pub(crate) fn work<T, E, C, M>(
    job: Arc<Job<T, E>>,
    context: JobContext,
    started: Moment,
    factory: M,
) -> Box<dyn CpuWork>
where
    T: Send + 'static,
    E: Send + 'static,
    C: CpuTask<T, E>,
    M: FnOnce(JobContext) -> C + Send + 'static,
{
    Box::new(TypedWork { job, context, started, factory: Some(factory), task: None })
}

#[derive(Clone, Copy)]
/// Restrict workers to the reviewed interactive/decode partition.
enum CpuLane {
    Interactive,
    Decode,
    Shared,
}

impl CpuLane {
    /// Select only the worker's independently admitted CPU classes.
    fn accepts(self, class: JobClass) -> bool {
        matches!(
            (self, class),
            (Self::Interactive | Self::Shared, JobClass::CpuInteractive)
                | (Self::Decode | Self::Shared, JobClass::CpuDecode)
        )
    }
}

/// Keep every worker handle until its observed join, including defensive destruction.
pub(crate) struct CpuPool {
    core: Arc<Core>,
    handles: Vec<JoinHandle<()>>,
}

impl CpuPool {
    /// Start one shared or two partitioned workers; join partial construction before failure.
    pub(crate) fn start(core: Arc<Core>, workers: usize) -> Result<Self, ExecutionError> {
        let lanes = match workers {
            1 => vec![CpuLane::Shared],
            2 => vec![CpuLane::Interactive, CpuLane::Decode],
            _ => {
                return Err(crate::runtime::error(std::io::Error::other("Invalid CPU worker cap")));
            }
        };
        let mut handles = Vec::with_capacity(lanes.len());
        for lane in lanes {
            let worker_core = core.clone();
            let exit = WorkerExit::enlist(&core, WorkerKind::Cpu);
            match thread::Builder::new()
                .name("sugarsubstitute-cpu".into())
                .spawn(move || worker(exit, worker_core, lane))
            {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    core.shutdown();
                    let mut failures =
                        vec![ExecutionError::new(Operation::Infrastructure, Box::new(error))];
                    for handle in handles {
                        if let Err(payload) = handle.join() {
                            failures.push(crate::runtime::error(std::io::Error::other(
                                panic_cause(payload).as_str().to_owned(),
                            )));
                        }
                    }
                    return Err(crate::runtime::aggregate_errors(failures));
                }
            }
        }
        Ok(Self { core, handles })
    }
    /// Join all workers and retain every failure after all handles have been observed.
    pub(crate) fn finish(mut self) -> Result<(), ExecutionError> {
        self.join()
    }
    /// Consume joins independently so an earlier failure cannot prevent later cleanup.
    fn join(&mut self) -> Result<(), ExecutionError> {
        let mut failures = Vec::new();
        for handle in self.handles.drain(..) {
            if let Err(payload) = handle.join() {
                failures.push(ExecutionError::new(
                    Operation::Infrastructure,
                    Box::new(std::io::Error::other(panic_cause(payload).as_str().to_owned())),
                ));
            }
        }
        if failures.is_empty() { Ok(()) } else { Err(crate::runtime::aggregate_errors(failures)) }
    }
}

impl Drop for CpuPool {
    fn drop(&mut self) {
        if !self.handles.is_empty() {
            self.core.shutdown();
            if let Err(error) = self.join() {
                tracing::error!(operation = ?error.operation(), "CPU worker teardown failed");
            }
        }
    }
}

/// Transfer admitted work into its worker while supervising private factory construction.
fn start(queued: QueuedJob) -> Option<Box<dyn CpuWork>> {
    let started = queued.context.now();
    if let Some(reason) = stop_reason(&queued.context) {
        stop_queued(queued, reason);
        return None;
    }
    match queued.work {
        Work::Cpu(factory) => {
            match catch_unwind(AssertUnwindSafe(|| factory(queued.context.clone(), started))) {
                Ok(work) => Some(work),
                Err(payload) => {
                    queued.job.panicked(&queued.context, started, panic_cause(payload));
                    None
                }
            }
        }
        other => {
            let cause = dispose(other)
                .err()
                .unwrap_or_else(|| PanicCause::new("Non-CPU work reached CPU worker".into()));
            queued.job.panicked(&queued.context, started, cause);
            None
        }
    }
}

/// Alternate retirement and cancellation cleanup without consuming active lane slots.
fn cleanup(core: &Core, lane: CpuLane, retired_first: bool) -> bool {
    if retired_first && let Some(retired) = core.take_retired_cpu(|class| lane.accepts(class)) {
        retire(retired);
        return true;
    }
    if let Some(queued) = core.take_stopped_cpu(|class| lane.accepts(class)) {
        if let Some(reason) = stop_reason(&queued.context) {
            stop_queued(queued, reason);
        } else {
            let cause = dispose(queued.work).err().unwrap_or_else(|| {
                PanicCause::new("Stopped CPU admission lost its stop reason".into())
            });
            queued.job.panicked(&queued.context, queued.context.now(), cause);
        }
        return true;
    }
    if let Some(retired) = core.take_retired_cpu(|class| lane.accepts(class)) {
        retire(retired);
        return true;
    }
    false
}

/// Hold each class slot across yields and grant both active classes one step per round.
/// The exit guard is the first parameter, so it drops after every owned slot and announces return.
fn worker(_exit: WorkerExit, core: Arc<Core>, lane: CpuLane) {
    let mut interactive: Option<Box<dyn CpuWork>> = None;
    let mut decode: Option<Box<dyn CpuWork>> = None;
    loop {
        // Cleanup shares four items per round before either active class takes a step.
        for index in 0..4 {
            if !cleanup(&core, lane, index % 2 == 0) {
                break;
            }
        }
        for (class, slot) in
            [(JobClass::CpuInteractive, &mut interactive), (JobClass::CpuDecode, &mut decode)]
        {
            if slot.is_none()
                && lane.accepts(class)
                && let Some(queued) = core.take(|candidate| candidate == class)
            {
                *slot = start(queued);
            }
        }
        if let Some(work) = &mut interactive
            && matches!(work.step(), CpuProgress::Stopped)
        {
            interactive = None;
        }
        if let Some(work) = &mut decode
            && matches!(work.step(), CpuProgress::Stopped)
        {
            decode = None;
        }
        if interactive.is_some() || decode.is_some() {
            continue;
        }
        let state = lock(&core.state);
        if state.stopping && state.settled() {
            break;
        }
        if state.queue.iter().any(|queued| lane.accepts(queued.context.spec().class))
            || state.retired.iter().any(|retired| retired.accepts(|class| lane.accepts(class)))
        {
            drop(state);
            continue;
        }
        drop(match core.cpu_changed.wait(state) {
            Ok(state) => state,
            Err(error) => error.into_inner(),
        });
    }
}

#[cfg(test)]
#[path = "cpu_tests.rs"]
mod tests;
