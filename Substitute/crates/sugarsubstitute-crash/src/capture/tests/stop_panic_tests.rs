// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain an earlier owned stop cause before the subsequent retirement can unwind.
use super::{loan_tests::*, *};
use sugarsubstitute_contracts::crash::CaptureOperation;

fn prepared(rig: &Rig, port: &Arc<FakePort>) -> TestResult<WindowsCrashCapture> {
    port.healthy.store(1, Ordering::Release);
    port.release.open();
    let mut capture = start(rig, port)?;
    assert!(matches!(terminal(rig, &mut capture)?, JobOutcome::Success(())));
    Ok(capture)
}
fn original_stop(port: &FakePort) -> CrashError {
    let cause =
        CrashError::new(CaptureOperation::Stop, std::io::Error::other("original-stop-failure"));
    *lock(&port.stop_failure) = Some(cause.clone());
    cause
}
fn retained(
    rig: &Rig,
    port: &FakePort,
    shared: &Shared,
    cause: &CrashError,
    original: usize,
) -> TestResult {
    let state = lock(&shared.native);
    let first = state.errors.iter().flatten().next().ok_or("Original stop cause missing")?;
    assert!(first.same_cause(cause));
    assert_eq!(state.errors.iter().flatten().count(), 1);
    assert!(!state.borrowed && !state.retired);
    assert_eq!(state.close_deadline, Some(Moment::from_nanos(10_000_000_000)));
    drop(state);
    assert_eq!(lease_address(shared)?, original);
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    assert!(rig.scope.snapshot().reserved_bytes >= crate::input::STORAGE_BYTES as u64);
    Ok(())
}

#[test]
fn stop_failure_survives_later_native_retirement_panic_and_checked_close() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let mut capture = prepared(&rig, &port)?;
    let shared = capture.shared.as_ref().ok_or("Original Shared missing")?.clone();
    let original = lease_address(&shared)?;
    let first = original_stop(&port);
    *lock(&port.retire_probe) = Some(Box::new(|_| {
        std::panic::resume_unwind(Box::new("original-retire-after-stop".to_owned()));
    }));
    let deadline = Moment::from_nanos(10_000_000_000);
    capture.submit_close(&rig.client, &rig.scope, close_spec(3, deadline)?)?;
    let mut close = capture.close.take().ok_or("Actual close job missing")?;
    let JobOutcome::Panicked(panic) = rig.completion(&mut close)?.outcome else {
        return Err("Original native retirement panic missing".into());
    };
    assert_eq!(panic.as_str(), "original-retire-after-stop");
    retained(&rig, &port, &shared, &first, original)?;
    *lock(&port.stop_failure) = None;
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    capture.submit_close(&rig.client, &cleanup, close_spec(4, deadline)?)?;
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    assert_eq!(port.retire_address.load(Ordering::Acquire), original);
    assert!(capture.errors().any(|cause| cause.same_cause(&first)));
    Ok(())
}

#[test]
fn stop_failure_survives_later_fatal_retirement_clock_panic_with_original_owner() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let mut capture = prepared(&rig, &port)?;
    let shared = capture.shared.as_ref().ok_or("Original Shared missing")?.clone();
    let original = lease_address(&shared)?;
    let first = original_stop(&port);
    let (unlocked, observed) = arm(
        &rig,
        shared.clone(),
        &port.retire_probe,
        Box::new(|| {
            std::panic::resume_unwind(Box::new("original-clock-after-stop".to_owned()));
        }),
    );
    capture.submit_close(
        &rig.client,
        &rig.scope,
        close_spec(3, Moment::from_nanos(10_000_000_000))?,
    )?;
    observed.wait()?;
    let mut close = capture.close.take().ok_or("Actual close job missing")?;
    let JobOutcome::Panicked(panic) =
        rig.fatal_completion(&mut close, "original-clock-after-stop")?.outcome
    else {
        return Err("Original fatal provider panic missing".into());
    };
    assert_eq!(panic.as_str(), "original-clock-after-stop");
    assert!(unlocked.load(Ordering::Acquire));
    retained(&rig, &port, &shared, &first, original)?;
    assert!(rig.client.now().is_err());
    Ok(())
}
