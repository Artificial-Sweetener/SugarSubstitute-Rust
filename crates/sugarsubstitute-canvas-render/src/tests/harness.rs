// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise image work on the real executor with an injected deterministic clock.
use std::{
    marker::PhantomData,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_image_ops::ImageError;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
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
impl<F, T> CpuTask<T, ImageError> for Task<F, T>
where
    F: FnOnce(&JobContext) -> Result<T, ImageError> + Send + 'static,
    T: Send + 'static,
{
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<T>, ImageError> {
        self.run.take().ok_or(ImageError::Invalid("Test task already finished"))?(context)
            .map(CpuStep::Complete)
    }
}

/// Real execution admission; the condition wait is a bounded external completion boundary.
pub(super) fn run<T: Send + 'static>(
    bytes: u64,
    action: impl FnOnce(&JobContext) -> Result<T, ImageError> + Send + 'static,
) -> TestResult<T> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::ImageDecode,
        class: JobClass::CpuDecode,
        deadline: None,
        reservation_bytes: bytes,
        replacement: None,
    };
    let mut job =
        client.submit_cpu(&scope, spec, |_| Task { run: Some(action), output: PhantomData })?;
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
        JobOutcome::Failure(cause) => Err(cause.into()),
        other => Err(std::io::Error::other(format!(
            "Unexpected image execution outcome: {:?}",
            other.kind()
        ))
        .into()),
    }
}
