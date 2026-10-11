// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test real admission/protocol/retirement with controlled external event effects.
use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

struct Boundary {
    signals: Mutex<Option<(Arc<Signal>, Arc<Signal>)>>,
    started: Gate,
    retired: Gate,
    close_release: Gate,
    hold_close: AtomicBool,
    synchronous_ready: bool,
    fail_start: bool,
    challenge_fail: AtomicBool,
    release_fail: AtomicBool,
    retirement_failures: AtomicUsize,
    panic_start: AtomicBool,
    panic_drop: AtomicBool,
    challenges: AtomicUsize,
    hold_challenge: AtomicBool,
    challenge_entered: Gate,
    challenge_continue: Gate,
    releases: AtomicUsize,
    closes: AtomicUsize,
}
impl Boundary {
    fn new(synchronous_ready: bool, fail_start: bool) -> Arc<Self> {
        Arc::new(Self {
            signals: Mutex::new(None),
            started: Gate::default(),
            retired: Gate::default(),
            close_release: Gate::default(),
            hold_close: AtomicBool::new(false),
            synchronous_ready,
            fail_start,
            challenge_fail: AtomicBool::new(false),
            release_fail: AtomicBool::new(false),
            retirement_failures: AtomicUsize::new(0),
            panic_start: AtomicBool::new(false),
            panic_drop: AtomicBool::new(false),
            challenges: AtomicUsize::new(0),
            hold_challenge: AtomicBool::new(false),
            challenge_entered: Gate::default(),
            challenge_continue: Gate::default(),
            releases: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
        })
    }
    fn signal(&self, response: bool) -> TestResult {
        let signals = lock(&self.signals).clone().ok_or("Fixture waits not registered")?;
        if response {
            signals.1.complete()
        } else {
            signals.0.complete()
        }
        Ok(())
    }
}
struct Lease(Arc<Boundary>);
fn error(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(stage, std::io::Error::from_raw_os_error(5))
}
impl ParentLease for Lease {
    fn start(
        &mut self,
        ready: Arc<Signal>,
        response: Arc<Signal>,
    ) -> Result<[usize; 4], NativeObservationError> {
        *lock(&self.0.signals) = Some((ready.clone(), response));
        if self.0.synchronous_ready {
            ready.complete();
        }
        self.0.started.open();
        if self.0.panic_start.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("fixture parent start panic".to_owned()));
        }
        if self.0.fail_start {
            Err(error("partial_fixture_registration"))
        } else {
            Ok([11, 12, 13, 14])
        }
    }
    fn challenge(&mut self) -> Result<(), NativeObservationError> {
        self.0.challenges.fetch_add(1, Ordering::AcqRel);
        if self.0.hold_challenge.load(Ordering::Acquire) {
            self.0.challenge_entered.open();
            self.0.challenge_continue.wait().map_err(|cause| {
                NativeObservationError::new(
                    "fake_challenge",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        if self.0.challenge_fail.load(Ordering::Acquire) {
            Err(error("signal_fixture_challenge"))
        } else {
            self.0.signal(true).map_err(|cause| {
                NativeObservationError::new(
                    "fake_response",
                    std::io::Error::other(cause.to_string()),
                )
            })
        }
    }
    fn validate_observation(&self) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn release(&mut self) -> Result<(), NativeObservationError> {
        self.0.releases.fetch_add(1, Ordering::AcqRel);
        if self.0.release_fail.load(Ordering::Acquire) {
            Err(error("signal_fixture_release"))
        } else {
            Ok(())
        }
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        self.0.retired.open();
        if self.0.hold_close.load(Ordering::Acquire) {
            self.0.close_release.wait().map_err(|cause| {
                NativeObservationError::new("fake_close", std::io::Error::other(cause.to_string()))
            })?;
        }
        if self
            .0
            .retirement_failures
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(error("unregister_fixture_ready"));
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

fn scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?)
}
fn specification(id: u64) -> TestResult<JobSpec> {
    let mut spec = spec(id, JobClass::NativeObservation, MIN_FIXTURE_SESSION_BYTES)?;
    spec.operation = Operation::NativeLifecycle;
    spec.deadline = Some(Moment::from_nanos(1_000));
    Ok(spec)
}
fn session(rig: &Rig, scope: &Scope, boundary: &Arc<Boundary>) -> TestResult<FixtureSession> {
    Ok(reserve(
        &rig.client.core,
        scope,
        specification(1)?,
        NativeCorrelation::new(1)?,
        Box::new(Lease(boundary.clone())),
    )?)
}
fn fact(rig: &Rig, control: &FixtureSessionControl, expected: FixtureObservation) -> TestResult {
    loop {
        let observed = rig.wake.count();
        while let Some(fact) = control.take_observation() {
            if fact == expected {
                return Ok(());
            }
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        while let Some(fact) = control.take_observation() {
            if fact == expected {
                return Ok(());
            }
        }
        rig.wake.wait_after(observed)?;
    }
}

#[test]
fn actual_ready_challenge_response_release_are_distinct_from_exit() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Prepared)?;
    let artifact = session.control.take_child_events()?.ok_or("Missing prepared artifact")?;
    assert_eq!(artifact.handles(), [11, 12, 13, 14]);
    assert_eq!(
        session.control.take_child_events().err(),
        Some(FixtureSessionError::AlreadyRequested)
    );
    assert_eq!(session.control.request_challenge(), Err(FixtureSessionError::NotReady));
    assert!(session.completion.try_take_completion().is_none());
    assert!(rig.client.drain_completions(NonZeroUsize::MIN)?.notifications.is_empty());
    boundary.signal(false)?;
    fact(&rig, &session.control, FixtureObservation::Ready)?;
    session.control.request_challenge()?;
    assert_eq!(session.control.request_challenge(), Err(FixtureSessionError::AlreadyRequested));
    fact(&rig, &session.control, FixtureObservation::Responded)?;
    session.control.request_release()?;
    assert!(matches!(
        session.control.request_release(),
        Err(FixtureSessionError::AlreadyRequested | FixtureSessionError::Closed)
    ));
    drop(artifact);
    let completion = rig.completion(&mut session.completion)?;
    assert!(matches!(
        completion.outcome,
        JobOutcome::Success(FixtureSessionReport {
            ready: true,
            challenge_sent: true,
            responded: true,
            release_sent: true,
            ..
        })
    ));
    assert_eq!(boundary.challenges.load(Ordering::Acquire), 1);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 1);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn cancellation_retains_creation_pin_and_callbacks_until_actual_retirement() -> TestResult {
    let mut rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Prepared)?;
    let artifact = session.control.take_child_events()?.ok_or("Missing artifact")?;
    boundary.hold_close.store(true, Ordering::Release);
    session.control.cancel();
    assert_eq!(session.control.request_release(), Err(FixtureSessionError::Closed));
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_FIXTURE_SESSION_BYTES);
    drop(artifact);
    boundary.retired.wait()?;
    assert_eq!(boundary.releases.load(Ordering::Acquire), 0);
    rig.owner.request_shutdown(Moment::from_nanos(0));
    let shutdown = rig.owner.poll_shutdown()?;
    assert!(!shutdown.settled);
    assert!(shutdown.unresolved_job_ids.contains(&JobId::new(1)?));
    boundary.close_release.open();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn synchronous_ready_during_registration_publishes_only_after_preparation() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(true, false);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Prepared)?;
    fact(&rig, &session.control, FixtureObservation::Ready)?;
    session.control.request_release()?;
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}

#[test]
fn minimum_memory_deadline_and_scope_reject_before_native_effects() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let mut invalid = specification(1)?;
    invalid.reservation_bytes -= 1;
    assert!(matches!(
        reserve(
            &rig.client.core,
            &scope,
            invalid,
            NativeCorrelation::new(1)?,
            Box::new(Lease(boundary.clone()))
        ),
        Err(AdmissionError::NativeReservationTooSmall { .. })
    ));
    assert!(
        reserve(
            &rig.client.core,
            &rig.scope,
            specification(1)?,
            NativeCorrelation::new(1)?,
            Box::new(Lease(boundary.clone()))
        )
        .is_err()
    );
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    let mut missing = specification(1)?;
    missing.deadline = None;
    assert!(matches!(
        reserve(
            &rig.client.core,
            &scope,
            missing,
            NativeCorrelation::new(1)?,
            Box::new(Lease(boundary.clone()))
        ),
        Err(AdmissionError::DeadlineRequired)
    ));
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    let mut accepted = session(&rig, &scope, &boundary)?;
    accepted.control.cancel();
    assert!(matches!(rig.completion(&mut accepted.completion)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.0.panic_drop.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("fixture parent destructor panic".to_owned()));
        }
    }
}
#[path = "parent_failure_tests.rs"]
mod failure;
