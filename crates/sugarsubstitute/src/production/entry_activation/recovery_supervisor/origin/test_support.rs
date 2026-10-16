// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real execution ownership with injected clock and notification synchronization.
use super::*;
use std::{num::NonZeroUsize, sync::Condvar, time::Duration};
use sugarsubstitute_execution::ExecutionOwner;
pub(super) type TestError = Box<dyn Error + Send + Sync>;
pub(super) struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
pub(super) struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.count) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(super) fn observe<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, TestError> {
    loop {
        let before = *lock(&wake.count);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(lock(&wake.count), Duration::from_secs(10), |count| {
                *count == before
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && *count == before {
            return Err("actual origin worker completion stalled".into());
        }
    }
}
pub(super) fn owner() -> Result<(ExecutionOwner, ExecutionClient, Arc<Wake>, Scope), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    Ok((owner, client, wake, scope))
}
pub(super) fn spec(id: JobId, bytes: u64) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: bytes,
        replacement: None,
    }
}
pub(super) fn success<T>(done: JobCompletion<T, TestError>) -> Result<T, TestError> {
    match done.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(cause),
        _ => Err("origin fixture worker did not succeed".into()),
    }
}

/// Drain the actual Core batches into an owner callback under the same finite fixture wait.
pub(super) fn capture_until(
    client: &ExecutionClient,
    wake: &Wake,
    mut capture: impl FnMut(&CompletionBatch) -> Result<bool, TestError>,
) -> Result<(), TestError> {
    loop {
        let before = *lock(&wake.count);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        if capture(&batch)? {
            return Ok(());
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(lock(&wake.count), Duration::from_secs(10), |count| {
                *count == before
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && *count == before {
            return Err("actual origin owner capture stalled".into());
        }
    }
}
