// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run custody tests through the actual paired fixed symbol admission and observed terminals.
use crate::ClaimError;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(crate) struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
pub(crate) struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(crate) fn completion<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, Box<dyn std::error::Error>> {
    loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == old,
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if timeout.timed_out() && *count == old {
            return Err("Actual symbol fixture terminal absent".into());
        }
    }
}
/// Run an original paired main without issuing any selected-symbol authority.
pub(crate) fn admitted(
    work: impl FnOnce(JobContext) -> Result<(), ClaimError> + Send + 'static,
) -> TestResult {
    paired(move |_, context| work(context))
}

/// Issue only the actual running paired main's original selected-symbol admission.
pub(crate) fn symbol(
    work: impl FnOnce(
        sugarsubstitute_execution::SymbolBundleAdmission,
        JobContext,
    ) -> Result<(), ClaimError>
    + Send
    + 'static,
) -> TestResult {
    paired(move |client, context| {
        let admission = client
            .symbol_bundle_admission(&context)
            .map_err(|cause| ClaimError::new("fixture_actual_symbol_admission", cause))?;
        work(admission, context)
    })
}

/// Keep the existing main and unbegun cleanup terminal under one actual execution owner.
fn paired(
    work: impl FnOnce(&ExecutionClient, JobContext) -> Result<(), ClaimError> + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let main_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let cleanup_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let main_spec = JobSpec {
        id: JobId::new(2)?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: 8_159_232,
        replacement: None,
    };
    let cleanup_spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureLifecycle,
        deadline: None,
        reservation_bytes: 32_768,
        replacement: None,
    };
    let permit = client
        .reserve_symbol_cleanup(&cleanup_scope, cleanup_spec, &main_scope, main_spec, |_| {
            Ok::<_, ClaimError>(())
        })
        .map_err(|rejection| rejection.error)?;
    let worker = client.clone();
    let mut main = client
        .submit_symbol_helper(&main_scope, main_spec, &permit, move |context| {
            work(&worker, context)
        })
        .map_err(|rejection| rejection.error)?;
    let main_done = completion(&client, &wake, &mut main);
    // The unbegun cleanup still has one actually owned terminal on every main outcome.
    let mut cleanup = permit.cancel_and_observe();
    let cleanup_done = completion(&client, &wake, &mut cleanup)?;
    assert!(matches!(cleanup_done.outcome, JobOutcome::Cancelled));
    let main_done = main_done?;
    match main_done.outcome {
        JobOutcome::Success(()) => Ok(()),
        other => Err(format!("Actual symbol fixture failed: {other:?}").into()),
    }
}

/// Submit the actual fixed capture class and obtain only its Core-produced admission.
pub(crate) fn capture(
    work: impl FnOnce(
        sugarsubstitute_execution::CaptureBundleAdmission,
        JobContext,
    ) -> Result<(), ClaimError>
    + Send
    + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(3)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: 4_194_304,
        replacement: None,
    };
    let worker = client.clone();
    let mut job = client.submit_blocking(&scope, spec, move |context| {
        let admission = worker
            .capture_bundle_admission(&context)
            .map_err(|cause| ClaimError::new("fixture_actual_capture_admission", cause))?;
        work(admission, context)
    })?;
    let done = completion(&client, &wake, &mut job)?;
    match done.outcome {
        JobOutcome::Success(()) => Ok(()),
        other => Err(format!("Actual capture fixture failed: {other:?}").into()),
    }
}
