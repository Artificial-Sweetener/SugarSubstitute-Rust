// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A supplied timer-poll failure remains original and cannot imply elapsed time.
use super::*;
use std::error::Error;
#[test]
fn provider_future_poll_failure_latches_original_cancels_owned_work_and_joins_workers() -> TestResult
{
    let clock = ToggleClock::new();
    clock.clock.advance(17);
    let wake = Arc::new(Wake::default());
    let mut owner = crate::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let mut finite = client.submit_blocking(
        &scope,
        spec(1, JobClass::InfrastructureOrdinary, 4096)?,
        move |_| {
            worker_entered.open();
            worker_release.wait().map_err(|e| e.to_string())
        },
    )?;
    entered.wait()?;
    let provider = clock.clone();
    let mut timer = client.submit_async(
        &scope,
        spec(2, JobClass::IoOrdinary, 4096)?,
        move |context| async move {
            provider.poll_failure.store(true, Ordering::Release);
            context.sleep_until(Moment::from_nanos(777)).await;
            Ok::<(), ()>(())
        },
    )?;
    let completion = loop {
        let observed = wake.count();
        if let Some(value) = timer.try_take_completion() {
            break value;
        }
        wake.wait_after(observed)?;
    };
    assert!(matches!(completion.outcome, JobOutcome::Panicked(_)));
    clock.permanent.store(true, Ordering::Release);
    let cause = client.now().err().ok_or("Poll failure returned public metadata")?;
    assert!(
        cause.source().is_some_and(|cause| cause
            .to_string()
            .contains("original provider future poll failure"))
    );
    owner.request_shutdown(Moment::from_nanos(17));
    let retained = owner
        .poll_shutdown()
        .err()
        .ok_or("Taking the timer completion erased its independent failure notice")?;
    assert!(
        retained.source().is_some_and(|cause| cause
            .to_string()
            .contains("original provider future poll failure"))
    );
    let pending = owner.poll_shutdown()?;
    assert!(!pending.settled);
    assert!(!pending.deadline_expired);
    assert!(finite.try_take_completion().is_none());
    assert!(scope.snapshot().reserved_bytes >= 4096);
    release.open();
    let finite_completion = loop {
        let observed = wake.count();
        if let Some(value) = finite.try_take_completion() {
            break value;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(value) = finite.try_take_completion() {
            break value;
        }
        wake.wait_after(observed)?;
    };
    assert!(matches!(finite_completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    drop(finite_completion);
    drop(timer);
    drop(finite);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(clock.calls_after_failure.load(Ordering::Acquire), 0);
    assert_eq!(clock.clock.expired_sleep_calls.load(Ordering::Acquire), 0);
    Ok(())
}
