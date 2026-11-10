// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real execution ownership with injected clock and notification synchronization.
use super::*;
use crate::completion_wake::Wake;
use std::num::NonZeroUsize;
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
pub(super) fn observe<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, TestError> {
    wake.completion(client, job).map_err(|cause| cause.to_string().into())
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

/// Drain the actual Core batches into an owner callback until it reports completion.
pub(super) fn capture_until(
    client: &ExecutionClient,
    wake: &Wake,
    mut capture: impl FnMut(&CompletionBatch) -> Result<bool, TestError>,
) -> Result<(), TestError> {
    wake.until(|| {
        let batch = match client.drain_completions(NonZeroUsize::MIN) {
            Ok(batch) => batch,
            Err(cause) => return Some(Err(cause.into())),
        };
        match capture(&batch) {
            Ok(false) => None,
            Ok(true) => Some(Ok(())),
            Err(cause) => Some(Err(cause)),
        }
    })
}
