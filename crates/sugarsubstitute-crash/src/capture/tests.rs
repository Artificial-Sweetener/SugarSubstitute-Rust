// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real execution admission and retained owner state with a fake native boundary.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Condvar,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, windows::NativeCorrelation};

mod support;
use support::*;

mod deadline_tests;
mod registered_tests;

#[test]
fn inert_owner_and_input_reject_unbounded_capacities_and_private_diagnostics() -> TestResult {
    let owner = WindowsCrashCapture::new(coordinator()?);
    assert_eq!(owner.snapshot().state, CaptureState::Dormant);
    let mut path = std::path::PathBuf::with_capacity(crate::input::MAX_UNITS * 3 + 1);
    path.push(r"E:\private\client.dll");
    assert!(
        CaptureStartInput::new(
            path,
            r"E:\private\handler.exe".into(),
            r"D:\private\capture-root".into(),
            [0; 32],
            [0; 32],
            "build"
        )
        .is_err()
    );
    let cause = CrashError::new(
        sugarsubstitute_contracts::crash::CaptureOperation::Runtime,
        std::io::Error::other("private-secret-path"),
    );
    assert!(!format!("{cause:?} {cause}").contains("private-secret"));
    assert!(
        std::error::Error::source(&cause)
            .is_some_and(|source| source.to_string().contains("private-secret"))
    );
    Ok(())
}

#[test]
fn future_budget_rejects_before_native_or_watch_consumption() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let watch = rig.watch(1)?;
    let budget = CaptureBudget::new(Moment::from_nanos(1))?;
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
        .ok_or("Future budget accepted")?;
    let (reason, watch, recovered) = rejection.into_parts()?;
    assert!(matches!(reason, CrashAdmissionError::InvalidSpecification));
    assert_eq!(recovered.build, "test-build");
    assert_eq!(port.created.load(Ordering::Acquire), 0);
    assert_eq!(rig.native_scope.snapshot().terminal_unobserved, 0);
    let mut retired = watch.cancel_unbound();
    assert!(matches!(rig.completion(&mut retired)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn rejected_execution_admission_latches_original_budget_and_returns_exact_input() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let budget = CaptureBudget::new(Moment::from_nanos(0))?;
    rig.scope.cancel();
    let rejection = capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(2, budget.until())?,
            budget,
            rig.watch(1)?,
            input()?,
        )
        .err()
        .ok_or("Cancelled scope accepted")?;
    let (reason, watch, input) = rejection.into_parts()?;
    assert!(matches!(reason, CrashAdmissionError::Execution(_)));
    rig.clock.0.store(1, Ordering::Release);
    let renewed = CaptureBudget::new(Moment::from_nanos(1))?;
    let rejection = capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(2, renewed.until())?,
            renewed,
            watch,
            input,
        )
        .err()
        .ok_or("Renewed budget accepted")?;
    let (_, watch, _) = rejection.into_parts()?;
    assert_eq!(capture.start_budget, Some(budget));
    assert_eq!(port.created.load(Ordering::Acquire), 0);
    let mut retired = watch.cancel_unbound();
    assert!(matches!(rig.completion(&mut retired)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn late_cancellation_retains_actual_lease_until_fresh_scope_checked_close() -> TestResult {
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
    rig.scope.cancel();
    assert_eq!(capture.snapshot().state, CaptureState::Starting);
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    assert!(rig.scope.snapshot().reserved_bytes >= crate::input::STORAGE_BYTES as u64);
    port.release.open();
    rig.observe_until(&mut capture, |capture| capture.start.is_none() && capture.watch.is_none())?;
    assert_eq!(capture.snapshot().state, CaptureState::Degraded);
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    capture.submit_close(&rig.client, &cleanup, close_spec(3, budget.until())?)?;
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    assert_eq!(port.stopped.load(Ordering::Acquire), 1);
    assert_eq!(port.dropped.load(Ordering::Acquire), 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn queued_cancellation_never_constructs_native_boundary() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let budget = CaptureBudget::new(Moment::from_nanos(0))?;
    let watch = rig.watch(1)?;
    let blocked = Arc::new(Barrier::default());
    let release = Arc::new(Gate::default());
    let mut jobs = Vec::new();
    for id in [2, 3] {
        let blocked = blocked.clone();
        let release = release.clone();
        jobs.push(rig.client.submit_blocking(
            &rig.scope,
            close_spec(id, budget.until())?,
            move |_| -> Result<(), CrashError> {
                blocked.arrive();
                release.wait().map_err(|cause| {
                    CrashError::new(
                        sugarsubstitute_contracts::crash::CaptureOperation::Runtime,
                        cause,
                    )
                })
            },
        )?);
    }
    blocked.wait(2)?;
    capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(4, budget.until())?,
            budget,
            watch,
            input()?,
        )
        .map_err(|_| "Start rejected")?;
    rig.scope.cancel();
    release.open();
    for job in &mut jobs {
        assert!(matches!(rig.completion(job)?.outcome, JobOutcome::Cancelled));
    }
    rig.observe_until(&mut capture, |capture| capture.start.is_none() && capture.watch.is_none())?;
    assert_eq!(port.created.load(Ordering::Acquire), 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, crate::input::STORAGE_BYTES as u64);
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    capture.submit_close(&rig.client, &cleanup, close_spec(5, budget.until())?)?;
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    Ok(())
}

#[test]
fn failed_close_preserves_original_deadline_and_rejects_extension() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    port.release.open();
    port.fail_stop.store(1, Ordering::Release);
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
    let deadline = Moment::from_nanos(10_000_000_000);
    capture.submit_close(&rig.client, &rig.scope, close_spec(3, deadline)?)?;
    rig.observe_until(&mut capture, |capture| capture.close.is_none())?;
    assert_eq!(capture.close_deadline, Some(deadline));
    assert_eq!(capture.snapshot().state, CaptureState::Closing);
    assert!(matches!(
        capture.submit_close(
            &rig.client,
            &rig.scope,
            close_spec(4, Moment::from_nanos(11_000_000_000))?
        ),
        Err(CrashAdmissionError::InvalidSpecification)
    ));
    assert_eq!(port.stopped.load(Ordering::Acquire), 1);
    capture.submit_close(&rig.client, &rig.scope, close_spec(4, deadline)?)?;
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    assert!(capture.errors().count() >= 1);
    Ok(())
}
