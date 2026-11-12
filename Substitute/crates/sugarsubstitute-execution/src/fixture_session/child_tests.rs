// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise original deadline and child ownership through real execution lifetimes.
use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult};
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::execution::*;

struct Boundary {
    signals: Mutex<Option<(Arc<Signal>, Arc<Signal>)>>,
    ready: Gate,
    response: Gate,
    retired: Gate,
    close_release: Gate,
    starts: AtomicUsize,
    closes: AtomicUsize,
    responses: AtomicUsize,
    preparation_entered: Gate,
    preparation_continue: Gate,
    hold_preparation: std::sync::atomic::AtomicBool,
    ready_signals: AtomicUsize,
    panic_start: std::sync::atomic::AtomicBool,
    retire_failures: AtomicUsize,
    fail_validation: bool,
    hold_close: bool,
}
impl Boundary {
    fn new(fail_validation: bool, hold_close: bool) -> Arc<Self> {
        Arc::new(Self {
            signals: Mutex::new(None),
            ready: Gate::default(),
            response: Gate::default(),
            retired: Gate::default(),
            close_release: Gate::default(),
            starts: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            responses: AtomicUsize::new(0),
            preparation_entered: Gate::default(),
            preparation_continue: Gate::default(),
            hold_preparation: std::sync::atomic::AtomicBool::new(false),
            ready_signals: AtomicUsize::new(0),
            panic_start: std::sync::atomic::AtomicBool::new(false),
            retire_failures: AtomicUsize::new(0),
            fail_validation,
            hold_close,
        })
    }
    fn signal(&self, release: bool) -> TestResult {
        let signals = lock(&self.signals).clone().ok_or("Child waits not registered")?;
        if release {
            signals.1.complete()
        } else {
            signals.0.complete()
        }
        Ok(())
    }
}
struct Lease(Arc<Boundary>);
impl ChildLease for Lease {
    fn start(
        &mut self,
        challenge: Arc<Signal>,
        release: Arc<Signal>,
    ) -> Result<(), NativeObservationError> {
        self.0.starts.fetch_add(1, Ordering::AcqRel);
        if self.0.panic_start.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("fixture child preparation panic".to_owned()));
        }
        if self.0.fail_validation {
            return Err(NativeObservationError::new(
                "fixture_child_clear_inherit",
                std::io::Error::from_raw_os_error(5),
            ));
        }
        *lock(&self.0.signals) = Some((challenge, release));
        if self.0.hold_preparation.load(Ordering::Acquire) {
            self.0.preparation_entered.open();
            self.0.preparation_continue.wait().map_err(|cause| {
                NativeObservationError::new(
                    "fake_prepare",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        Ok(())
    }
    fn ready(&mut self) -> Result<(), NativeObservationError> {
        self.0.ready_signals.fetch_add(1, Ordering::AcqRel);
        self.0.ready.open();
        Ok(())
    }
    fn response(&mut self) -> Result<(), NativeObservationError> {
        self.0.responses.fetch_add(1, Ordering::AcqRel);
        self.0.response.open();
        Ok(())
    }
    fn validate_observation(&self) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        self.0.retired.open();
        if self.0.hold_close {
            self.0.close_release.wait().map_err(|cause| {
                NativeObservationError::new(
                    "fake_child_close",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        if self
            .0
            .retire_failures
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(NativeObservationError::new(
                "unregister_fixture_challenge",
                std::io::Error::from_raw_os_error(6),
            ));
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

#[path = "child_failure_tests.rs"]
mod failure;

#[test]
fn cancellation_during_preparation_never_signals_ready() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    boundary.hold_preparation.store(true, Ordering::Release);
    let mut job = permit(&rig, &scope)?.bind_lease(Box::new(Lease(boundary.clone())));
    boundary.preparation_entered.wait()?;
    job.cancel();
    boundary.preparation_continue.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.ready_signals.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn preparation_crossing_deadline_never_signals_ready() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    boundary.hold_preparation.store(true, Ordering::Release);
    let mut job = permit(&rig, &scope)?.bind_lease(Box::new(Lease(boundary.clone())));
    boundary.preparation_entered.wait()?;
    rig.clock.advance(1_000);
    boundary.preparation_continue.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.ready_signals.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
fn scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?)
}
fn spec() -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(1)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::NativeObservation,
        deadline: None,
        reservation_bytes: MIN_FIXTURE_SESSION_BYTES,
        replacement: None,
    })
}
fn permit(rig: &Rig, scope: &Scope) -> TestResult<FixtureChildPermit> {
    Ok(reserve_child(
        &rig.client.core,
        scope,
        spec()?,
        NativeCorrelation::new(1)?,
        Moment::from_nanos(1_000),
    )?)
}

#[test]
fn inherited_roles_validate_shape_without_native_ownership_or_lossy_conversion() -> TestResult {
    assert!(InheritedFixtureEvents::new(0, 2, 3, 4).is_err());
    assert!(InheritedFixtureEvents::new(1, 2, 1, 4).is_err());
    assert!(InheritedFixtureEvents::new(u64::MAX, 2, 3, 4).is_err());
    assert_eq!(InheritedFixtureEvents::new(11, 12, 13, 14)?.handles, [11, 12, 13, 14]);
    Ok(())
}
#[test]
fn actual_child_response_follows_challenge_and_release_is_not_process_exit() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let mut job = permit(&rig, &scope)?.bind_lease(Box::new(Lease(boundary.clone())));
    boundary.ready.wait()?;
    assert_eq!(boundary.responses.load(Ordering::Acquire), 0);
    assert!(job.try_take_completion().is_none());
    boundary.signal(false)?;
    boundary.response.wait()?;
    assert_eq!(boundary.responses.load(Ordering::Acquire), 1);
    boundary.signal(true)?;
    let JobOutcome::Success(report) = rig.completion(&mut job)?.outcome else {
        return Err("Missing child protocol completion".into());
    };
    assert!(
        report.inheritance_cleared
            && report.ready_signalled
            && report.response_signalled
            && report.release_observed
    );
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn cancellation_before_bind_still_adopts_and_closes_inherited_owner() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let permit = permit(&rig, &scope)?;
    let original = permit.deadline();
    scope.cancel();
    rig.clock.advance(100);
    assert_eq!(permit.deadline(), original);
    let mut job = permit.bind_lease(Box::new(Lease(boundary.clone())));
    assert_eq!(job.spec().deadline, Some(original));
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.starts.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn failed_inherit_clear_never_signals_ready_and_retires_actual_owner() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(true, false);
    let mut job = permit(&rig, &scope)?.bind_lease(Box::new(Lease(boundary.clone())));
    let JobOutcome::Failure(error) = rig.completion(&mut job)?.outcome else {
        return Err("Failed inheritance accepted".into());
    };
    assert_eq!(error.stage(), "fixture_child_clear_inherit");
    assert!(lock(&boundary.signals).is_none());
    assert_eq!(boundary.responses.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn child_cancellation_keeps_handles_and_credit_until_close_returns() -> TestResult {
    let mut rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, true);
    let mut job = permit(&rig, &scope)?.bind_lease(Box::new(Lease(boundary.clone())));
    boundary.ready.wait()?;
    job.cancel();
    boundary.retired.wait()?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_FIXTURE_SESSION_BYTES);
    rig.owner.request_shutdown(Moment::from_nanos(0));
    assert!(!rig.owner.poll_shutdown()?.settled);
    boundary.close_release.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[path = "child_deadline_tests.rs"]
mod deadline;
