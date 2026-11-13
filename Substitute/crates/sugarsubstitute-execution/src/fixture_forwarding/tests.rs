// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise admitted forwarding with controlled prepare/close boundaries, not native calls.
use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;
struct Boundary {
    prepares: AtomicUsize,
    closes: AtomicUsize,
    fail: bool,
    entered: Gate,
    release: Gate,
    hold: AtomicBool,
}
impl Boundary {
    fn new(fail: bool) -> Arc<Self> {
        Arc::new(Self {
            prepares: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            fail,
            entered: Gate::default(),
            release: Gate::default(),
            hold: AtomicBool::new(false),
        })
    }
}
struct Lease(Arc<Boundary>);
impl ForwardLease for Lease {
    fn prepare(&mut self) -> Result<[usize; 4], NativeObservationError> {
        self.0.prepares.fetch_add(1, Ordering::AcqRel);
        if self.0.fail {
            Err(NativeObservationError::new(
                "partial_duplicate",
                std::io::Error::from_raw_os_error(5),
            ))
        } else {
            Ok([11, 12, 13, 14])
        }
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        self.0.entered.open();
        if self.0.hold.load(Ordering::Acquire) {
            self.0.release.wait().map_err(|cause| {
                NativeObservationError::new(
                    "controlled_close",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
fn inputs(rig: &Rig) -> TestResult<(Scope, JobSpec)> {
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_FIXTURE_SESSION_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    specification.deadline = Some(Moment::from_nanos(1000));
    Ok((scope, specification))
}
fn permit(rig: &Rig, scope: &Scope, spec: JobSpec) -> TestResult<FixtureForwardingPermit> {
    Ok(reserve_at(&rig.client.core, scope, spec, NativeCorrelation::new(1)?)?)
}
fn prepared(rig: &Rig, session: &FixtureForwardingSession) -> TestResult<FixtureForwardingEvents> {
    loop {
        let before = rig.wake.count();
        let _ = rig.client.drain_completions(NonZeroUsize::new(32).ok_or("budget")?)?;
        if let Some(events) = session.control.take_events()? {
            return Ok(events);
        }
        rig.wake.wait_after(before)?;
    }
}
#[test]
fn one_pin_retains_native_credit_and_memory_until_attribute_owner_releases_it() -> TestResult {
    let rig = Rig::new()?;
    let (scope, spec) = inputs(&rig)?;
    let boundary = Boundary::new(false);
    let mut session = permit(&rig, &scope, spec)?.bind_lease(Box::new(Lease(boundary.clone())));
    let events = prepared(&rig, &session)?;
    assert_eq!(events.handles(), [11, 12, 13, 14]);
    assert_eq!(
        session.control.take_events().err(),
        Some(crate::fixture_session::FixtureSessionError::AlreadyRequested)
    );
    session.control.cancel();
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, MIN_FIXTURE_SESSION_BYTES);
    drop(events);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn cancellation_before_late_bind_retains_and_closes_input_without_preparing() -> TestResult {
    let rig = Rig::new()?;
    let (scope, spec) = inputs(&rig)?;
    let boundary = Boundary::new(false);
    let permit = permit(&rig, &scope, spec)?;
    scope.cancel();
    assert_eq!(scope.snapshot().queued, 1);
    let mut session = permit.bind_lease(Box::new(Lease(boundary.clone())));
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn deadline_before_late_bind_cannot_release_input_admission_ownership() -> TestResult {
    let rig = Rig::new()?;
    let (scope, spec) = inputs(&rig)?;
    let boundary = Boundary::new(false);
    let permit = permit(&rig, &scope, spec)?;
    rig.clock.advance(1000);
    assert_eq!(scope.snapshot().queued, 1);
    let mut session = permit.bind_lease(Box::new(Lease(boundary.clone())));
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn cancelled_prepared_pin_cannot_be_taken_or_replaced() -> TestResult {
    let rig = Rig::new()?;
    let (scope, spec) = inputs(&rig)?;
    let boundary = Boundary::new(false);
    let mut session = permit(&rig, &scope, spec)?.bind_lease(Box::new(Lease(boundary.clone())));
    let events = prepared(&rig, &session)?;
    session.control.cancel();
    assert!(session.control.take_events().is_err());
    assert!(session.completion.try_take_completion().is_none());
    drop(events);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn partial_preparation_failure_closes_every_adopted_output_before_terminal() -> TestResult {
    let rig = Rig::new()?;
    let (scope, spec) = inputs(&rig)?;
    let boundary = Boundary::new(true);
    boundary.hold.store(true, Ordering::Release);
    let mut session = permit(&rig, &scope, spec)?.bind_lease(Box::new(Lease(boundary.clone())));
    boundary.entered.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().running, 1);
    boundary.release.open();
    let completion = rig.completion(&mut session.completion)?;
    assert!(
        matches!(completion.outcome,JobOutcome::Failure(ref cause) if cause.stage()=="partial_duplicate")
    );
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn released_launch_pin_completes_success_after_checked_closure() -> TestResult {
    let rig = Rig::new()?;
    let (scope, spec) = inputs(&rig)?;
    let boundary = Boundary::new(false);
    let mut session = permit(&rig, &scope, spec)?.bind_lease(Box::new(Lease(boundary.clone())));
    drop(prepared(&rig, &session)?);
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.prepared)
    );
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn forwarding_rejects_conflicting_deadline_before_sampling_or_accepting_handles() -> TestResult {
    let rig = Rig::new()?;
    let (scope, spec) = inputs(&rig)?;
    let calls = AtomicUsize::new(0);
    assert!(
        reserve(
            &rig.client.core,
            &scope,
            spec,
            NativeCorrelation::new(1)?,
            FixtureDeadline::new(1000, 1_000_000_000)?,
            || {
                calls.fetch_add(1, Ordering::AcqRel);
                Ok((1, 1_000_000_000))
            }
        )
        .is_err()
    );
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().queued, 0);
    Ok(())
}
