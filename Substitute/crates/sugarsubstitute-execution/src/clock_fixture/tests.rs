// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise original callback failure and independent observer progress through the real execution owner.
use super::*;
use crate::{
    ExecutionOwner,
    runtime_test_support::{Gate, TestResult, Wake, spec},
};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::execution::*;

#[derive(Clone, Copy, Eq, PartialEq)]
enum ObservationOrder {
    FailureFirst,
    CompletionFirst,
}
struct Release(Arc<Gate>, Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
        self.1.open();
    }
}
#[test]
fn callback_clock_regression_preserves_worker_panic_and_original_credit_before_public_failure()
-> TestResult {
    exercise(ObservationOrder::CompletionFirst)
}
#[test]
fn callback_clock_regression_is_globally_sticky_before_worker_terminal_without_affecting_observers()
-> TestResult {
    exercise(ObservationOrder::FailureFirst)
}
/// Consume healthy terminal/capacity notices before relying on the mailbox's coalesced wake.
fn healthy_completed<T: Send + 'static, E: Send + 'static>(
    client: &crate::ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> TestResult<JobCompletion<T, E>> {
    loop {
        let before = wake.count();
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        if batch.remaining > 0 {
            continue;
        }
        wake.wait_after(before)?;
    }
}
/// Wait for this real job's Core terminal without consuming a pending public failure first.
/// Core installs its exact completion under this same state fence before notifying changed.
fn terminal_before_notice<T: Send + 'static, E: Send + 'static>(
    client: &crate::ExecutionClient,
    job: &mut OwnedJob<T, E>,
) -> TestResult<JobCompletion<T, E>> {
    let core = client.core_for_test();
    let id = job.spec().id;
    let state = core
        .cpu_changed
        .wait_while(crate::scope::lock(&core.state), |state| {
            state.jobs.get(&id).is_some_and(|record| !matches!(record.phase, JobPhase::Terminal(_)))
        })
        .map_err(|cause| std::io::Error::other(cause.to_string()))?;
    let record = state.jobs.get(&id).ok_or("Original callback record disappeared")?;
    assert_eq!(record.scope_id, job.scope_id());
    assert_eq!(record.spec, job.spec());
    assert!(matches!(record.phase, JobPhase::Terminal(_)));
    drop(state);
    job.try_take_completion()
        .ok_or_else(|| "Actual Core terminal lacked original completion".into())
}
/// Retain and validate the genuine original fatal notice while acknowledging its separate wake.
fn failure_notice(client: &crate::ExecutionClient) -> TestResult<ExecutionError> {
    use std::error::Error;
    let cause = client
        .drain_completions(NonZeroUsize::MIN)
        .err()
        .ok_or("Original callback Regression notice was not retained")?;
    assert_eq!(cause.operation(), Operation::Infrastructure);
    assert!(
        cause
            .source()
            .is_some_and(|cause| cause.to_string().contains("Monotonic interval regressed"))
    );
    Ok(cause)
}
fn exercise(order: ObservationOrder) -> TestResult {
    let clock = CallbackRegressionClock::new(Moment::from_nanos(17));
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    assert!(!clock.arm_current_callback(Moment::from_nanos(17)));
    assert!(!clock.arm_current_callback(Moment::from_nanos(18)));
    assert_eq!(clock.requested_regression(), None);
    let armed = Arc::new(Gate::default());
    let invoke = Arc::new(Gate::default());
    let failed = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release = Release(invoke.clone(), release.clone());
    let callback_clock = clock.clone();
    let callback_armed = armed.clone();
    let callback_invoke = invoke.clone();
    let callback_failed = failed.clone();
    let callback_release = release.clone();
    let mut request = spec(1, JobClass::InfrastructureOrdinary, 4096)?;
    request.deadline = Some(Moment::from_nanos(100));
    let mut callback = client.submit_blocking(&scope, request, move |context| {
        assert!(callback_clock.arm_current_callback(Moment::from_nanos(16)));
        callback_armed.open();
        callback_invoke.wait().map_err(|cause| cause.to_string())?;
        let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| context.now()));
        callback_failed.open();
        if order == ObservationOrder::FailureFirst {
            callback_release.wait().map_err(|cause| cause.to_string())?;
        }
        match observed {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(_) => {
                Err::<(), _>("Actual callback did not encounter its clock regression".to_owned())
            }
        }
    })?;
    armed.wait()?;
    assert_eq!(clock.requested_regression(), Some(Moment::from_nanos(16)));
    assert!(!clock.arm_current_callback(Moment::from_nanos(15)));
    assert_eq!(clock.requested_regression(), Some(Moment::from_nanos(16)));
    assert_eq!(clock.regressed_samples(), 0);
    assert_eq!(client.now()?, Moment::from_nanos(17));
    // The blocked real callback occupies one worker; a second admitted class observes the
    // healthy epoch on the other worker before the targeted callback is released.
    let mut observer_spec = spec(2, JobClass::InfrastructureLifecycle, 4096)?;
    observer_spec.deadline = Some(Moment::from_nanos(100));
    let mut observer =
        client.submit_blocking(&scope, observer_spec, |context| Ok::<_, String>(context.now()))?;
    let observer_completion = healthy_completed(&client, &wake, &mut observer)?;
    assert!(
        matches!(observer_completion.outcome, JobOutcome::Success(now) if now == Moment::from_nanos(17))
    );
    assert_eq!(clock.regressed_samples(), 0);
    assert!(callback.try_take_completion().is_none());
    drop(observer_completion);
    drop(observer);
    // Taking a result removes its notice but does not rearm the mailbox's pending wake.
    let rearmed = client.drain_completions(NonZeroUsize::MIN)?;
    assert!(rearmed.notifications.is_empty());
    assert_eq!(rearmed.remaining, 0);
    invoke.open();
    let mut notice = None;
    if order == ObservationOrder::FailureFirst {
        failed.wait()?;
        assert!(callback.try_take_completion().is_none());
        notice = Some(failure_notice(&client)?);
        sticky_failure(&client)?;
        release.open();
    }
    let completion = terminal_before_notice(&client, &mut callback)?;
    if order == ObservationOrder::CompletionFirst {
        notice = Some(failure_notice(&client)?);
    }
    assert!(notice.is_some());
    assert!(matches!(completion.outcome, JobOutcome::Panicked(_)));
    assert_eq!(completion.spec, request);
    assert_eq!(completion.memory.bytes(), 4096);
    assert_eq!(clock.regressed_samples(), 1);
    sticky_failure(&client)?;
    assert_eq!(clock.regressed_samples(), 1);
    drop(callback);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(completion);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    // Notice ownership is independent of completion credit and keeps the original cause observable.
    let notice = notice.ok_or("Original callback failure notice lost")?;
    assert_eq!(notice.operation(), Operation::Infrastructure);
    drop(notice);
    Ok(())
}
fn sticky_failure(client: &crate::ExecutionClient) -> TestResult {
    use std::error::Error;
    let cause =
        client.now().err().ok_or("Original callback regression was not globally retained")?;
    assert_eq!(cause.operation(), Operation::Infrastructure);
    assert!(
        cause
            .source()
            .is_some_and(|cause| cause.to_string().contains("Monotonic interval regressed"))
    );
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    Ok(())
}
