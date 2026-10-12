// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Original clock/watch bounds are checked before effects and after late returns.
use super::*;

#[test]
fn foreign_or_registration_bounded_watch_rejects_before_native_effects() -> TestResult {
    let rig = Rig::new()?;
    let foreign = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let budget = CaptureBudget::new(Moment::from_nanos(0))?;
    let rejection = capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(2, budget.until())?,
            budget,
            foreign.watch(1)?,
            input()?,
        )
        .err()
        .ok_or("Foreign watch accepted")?;
    let (_, permit, _) = rejection.into_parts()?;
    let mut retired = permit.cancel_unbound();
    assert!(matches!(foreign.completion(&mut retired)?.outcome, JobOutcome::Cancelled));
    let mut spec = close_spec(1, budget.until())?;
    spec.class = JobClass::NativeObservation;
    let watch =
        rig.client.reserve_process_watch(&rig.native_scope, spec, NativeCorrelation::new(1)?)?;
    let rejection = capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(2, budget.until())?,
            budget,
            watch,
            input()?,
        )
        .err()
        .ok_or("Registration deadline accepted for lifelong watch")?;
    let (_, permit, _) = rejection.into_parts()?;
    let mut retired = permit.cancel_unbound();
    assert!(matches!(rig.completion(&mut retired)?.outcome, JobOutcome::Cancelled));
    assert_eq!(port.created.load(Ordering::Acquire), 0);
    assert_eq!(capture.start_budget, None);
    Ok(())
}

#[test]
fn late_original_deadline_retains_failed_native_facts_without_healthy_publication() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let budget = CaptureBudget::new(Moment::from_nanos(0))?;
    capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(2, budget.until())?,
            budget,
            rig.watch(1)?,
            input()?,
        )
        .map_err(|_| "Start rejected")?;
    port.entered.wait()?;
    rig.clock.0.store(budget.until().as_nanos(), Ordering::Release);
    port.release.open();
    rig.observe_until(&mut capture, |capture| capture.start.is_none() && capture.watch.is_none())?;
    assert_eq!(capture.snapshot().state, CaptureState::Degraded);
    assert_eq!(capture.start_budget, Some(budget));
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    assert!(capture.errors().any(|cause| {
        std::error::Error::source(cause)
            .is_some_and(|source| source.to_string().contains("controlled-registration-failure"))
    }));
    let deadline = Moment::from_nanos(75_000_000_000);
    capture.submit_close(&rig.client, &rig.scope, close_spec(3, deadline)?)?;
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    Ok(())
}

#[test]
fn partial_process_is_watch_bound_even_when_initial_identity_is_unavailable() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    port.release.open();
    port.partial_process.store(1, Ordering::Release);
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let budget = CaptureBudget::new(Moment::from_nanos(0))?;
    capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(2, budget.until())?,
            budget,
            rig.watch(1)?,
            input()?,
        )
        .map_err(|_| "Start rejected")?;
    rig.observe_until(&mut capture, |capture| capture.start.is_none() && capture.watch.is_none())?;
    assert_eq!(port.watch_binds.load(Ordering::Acquire), 1);
    assert_eq!(capture.snapshot().handler, None);
    assert_eq!(capture.snapshot().state, CaptureState::Degraded);
    capture.submit_close(&rig.client, &rig.scope, close_spec(3, budget.until())?)?;
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    Ok(())
}
