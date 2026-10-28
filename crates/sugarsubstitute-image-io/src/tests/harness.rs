// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise image work on the real executor with an injected deterministic clock.
use crate::ImportError;
use std::{
    marker::PhantomData,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct CancelState {
    reached: bool,
    released: bool,
}
/// Deterministically pause one test read while the owning execution job is cancelled.
#[derive(Default)]
pub(super) struct CancelPoint {
    state: Mutex<CancelState>,
    changed: Condvar,
}
impl CancelPoint {
    /// Stop the worker at an actual native-read boundary until the harness requests cancellation.
    pub(super) fn pause(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.reached = true;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
    fn wait_until_reached(&self) -> TestResult {
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(30), |state| !state.reached)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && !state.reached {
            return Err(std::io::Error::other("Image test did not reach cancellation point").into());
        }
        Ok(())
    }
    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.released = true;
        self.changed.notify_all();
    }
}
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
struct Task<F, T> {
    run: Option<F>,
    output: PhantomData<T>,
}
impl<F, T> CpuTask<T, ImportError> for Task<F, T>
where
    F: FnOnce(&JobContext) -> Result<T, ImportError> + Send + 'static,
    T: Send + 'static,
{
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<T>, ImportError> {
        self.run.take().ok_or(ImportError::Invalid("Test task already finished"))?(context)
            .map(CpuStep::Complete)
    }
}

/// Real execution admission; the condition wait is a bounded external completion boundary.
pub(super) fn run<T: Send + 'static>(
    bytes: u64,
    action: impl FnOnce(&JobContext) -> Result<T, ImportError> + Send + 'static,
) -> TestResult<T> {
    run_admitted(bytes, action, false, None)
}

/// Cancel an active image job while its test reader is paused inside one bounded operation.
pub(super) fn run_cancelled(
    bytes: u64,
    action: impl FnOnce(&JobContext, &CancelPoint) -> Result<(), ImportError> + Send + 'static,
) -> TestResult {
    let point = Arc::new(CancelPoint::default());
    let worker_point = point.clone();
    run_admitted(bytes, move |context| action(context, &worker_point), false, Some((point, ())))
}

/// Exercise native storage through the same execution-owned blocking boundary as production.
#[cfg(windows)]
pub(crate) fn run_storage<T: Send + 'static>(
    bytes: u64,
    action: impl FnOnce(&JobContext) -> Result<T, ImportError> + Send + 'static,
) -> TestResult<T> {
    run_admitted(bytes, action, true, None)
}

/// Cancel a storage worker while a bounded input read is in progress.
#[cfg(windows)]
pub(super) fn run_storage_cancelled(
    bytes: u64,
    action: impl FnOnce(&JobContext, &CancelPoint) -> Result<(), ImportError> + Send + 'static,
) -> TestResult {
    let point = Arc::new(CancelPoint::default());
    let worker_point = point.clone();
    run_admitted(bytes, move |context| action(context, &worker_point), true, Some((point, ())))
}

/// Share original admission and terminal observation without exporting a scheduler handle.
fn run_admitted<T: Send + 'static>(
    bytes: u64,
    action: impl FnOnce(&JobContext) -> Result<T, ImportError> + Send + 'static,
    storage: bool,
    cancelled: Option<(Arc<CancelPoint>, T)>,
) -> TestResult<T> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: if storage { Operation::Infrastructure } else { Operation::ImageDecode },
        class: if storage { JobClass::InfrastructureLifecycle } else { JobClass::CpuDecode },
        deadline: storage.then_some(Moment::from_nanos(1_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    };
    let mut job = if storage {
        client.submit_blocking(&scope, spec, |context| action(&context))?
    } else {
        client.submit_cpu(&scope, spec, |_| Task { run: Some(action), output: PhantomData })?
    };
    if let Some((point, _)) = &cancelled {
        if let Err(cause) = point.wait_until_reached() {
            point.release();
            return Err(cause);
        }
        job.cancel();
        point.release();
    }
    let outcome = loop {
        let revision = *wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let guard = wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (_guard, timeout) = wake
            .changed
            .wait_timeout_while(guard, Duration::from_secs(30), |v| *v == revision)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() {
            return Err(
                std::io::Error::other("Image execution did not reach a terminal outcome").into()
            );
        }
    };
    let maximum =
        NonZeroUsize::new(32).ok_or_else(|| std::io::Error::other("Invalid drain bound"))?;
    client.drain_completions(maximum)?;
    match outcome.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Cancelled => match cancelled {
            Some((_, value)) => Ok(value),
            None => Err(std::io::Error::other("Unexpected image execution cancellation").into()),
        },
        JobOutcome::Failure(cause) => {
            let mut observed: Option<&dyn std::error::Error> = Some(&cause);
            for depth in 0..16 {
                let Some(error) = observed else { break };
                eprintln!("Image test failure cause {depth}: {error}");
                observed = error.source();
            }
            Err(cause.into())
        }
        other => Err(std::io::Error::other(format!(
            "Unexpected image execution outcome: {:?}",
            other.kind()
        ))
        .into()),
    }
}
