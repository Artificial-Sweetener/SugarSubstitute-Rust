// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prevent supplied clocks and native wake callbacks on an identity-unrestored worker.
use super::*;
use crate::runtime_test_support::{ClockPort, ManualClock, Wake};
struct Audit {
    boundary: Arc<Boundary>,
    armed: AtomicBool,
    bad_clock: AtomicUsize,
    bad_wake: AtomicUsize,
}
impl Audit {
    fn contaminated_thread(&self) -> bool {
        self.armed.load(Ordering::Acquire)
            && self.boundary.restoration_failed.load(Ordering::Acquire)
            && lock(&self.boundary.owner_thread).is_some_and(|id| id == std::thread::current().id())
    }
}
struct ClockAudit {
    audit: Arc<Audit>,
    clock: Arc<ManualClock>,
}
impl Clock for ClockAudit {
    fn now(&self) -> Moment {
        if self.audit.contaminated_thread() {
            self.audit.bad_clock.fetch_add(1, Ordering::AcqRel);
        }
        ClockPort(self.clock.clone()).now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        ClockPort(self.clock.clone()).sleep_until(deadline)
    }
}
struct WakeAudit {
    audit: Arc<Audit>,
    wake: Arc<Wake>,
}
impl CompletionWake for WakeAudit {
    fn wake(&self) -> Result<(), ExecutionError> {
        if self.audit.contaminated_thread() {
            self.audit.bad_wake.fetch_add(1, Ordering::AcqRel);
        }
        self.wake.wake()
    }
}
#[test]
fn failed_restore_stops_native_clock_and_defers_wake_to_the_same_owned_supervisor() -> TestResult {
    let boundary = Boundary::new();
    let audit = Arc::new(Audit {
        boundary: boundary.clone(),
        armed: AtomicBool::new(false),
        bad_clock: AtomicUsize::new(0),
        bad_wake: AtomicUsize::new(0),
    });
    let clock = Arc::new(ManualClock::default());
    let wake = Arc::new(Wake::default());
    let owner = crate::runtime::ExecutionOwner::new(
        Arc::new(ClockAudit { audit: audit.clone(), clock: clock.clone() }),
        Arc::new(WakeAudit { audit: audit.clone(), wake: wake.clone() }),
    )?;
    let client = owner.client();
    let normal = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner, client, clock, wake, scope: normal };
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: Some(Moment::from_nanos(800)) },
        Some(&rig.scope),
        true,
    )?;
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let mut unrelated_spec = spec(2, JobClass::NativeObservation, 4096)?;
    unrelated_spec.operation = Operation::NativeLifecycle;
    let unrelated = crate::native::reserve(
        &rig.client.core,
        &scope,
        unrelated_spec,
        crate::native::NativeCorrelation::new(2)?,
    )?;
    audit.armed.store(true, Ordering::Release);
    boundary.fatal_message.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    boundary.retire_failures.store(1, Ordering::Release);
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    boundary.retiring.wait()?;
    rig.clock.advance(801);
    let state = lock(&rig.client.core.state);
    let record = state.jobs.get(&JobId::new(1)?).ok_or("Missing retained pipe record")?;
    assert_eq!(record.deadline, Some(Moment::from_nanos(800)));
    assert!(record.native_deadlines.as_ref().is_some_and(|slots| slots.stopped()));
    drop(state);
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Handshake)?;
    let JobOutcome::Failure(error) = super::failures::completion(&rig, &mut session)?.outcome
    else {
        return Err("Expected fatal identity failure".into());
    };
    assert!(error.requires_process_shutdown());
    assert_eq!(error.native_cause().stage(), "failed_identity_restore");
    assert_eq!(audit.bad_clock.load(Ordering::Acquire), 0);
    assert_eq!(audit.bad_wake.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert!(lock(&rig.client.core.state).stopping);
    let mut unrelated = unrelated.cancel_input();
    assert!(matches!(
        rig.completion(&mut unrelated)?.outcome,
        JobOutcome::Cancelled | JobOutcome::TimedOut
    ));
    Ok(())
}

#[test]
fn changed_message_identity_after_authentication_never_publishes_received_bytes() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    let first =
        fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Received { .. }))?;
    drop(first);
    session.control.verify_capability([7; 32], Moment::from_nanos(900))?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Authenticated(_)))?;
    boundary.reject_message.store(true, Ordering::Release);
    session.control.read(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900))?;
    let JobOutcome::Failure(error) = super::failures::completion(&rig, &mut session)?.outcome
    else {
        return Err("Expected changed message identity rejection".into());
    };
    assert_eq!(error.native_cause().stage(), "pipe_message_identity_mismatch");
    assert!(!error.requires_process_shutdown());
    assert_eq!(boundary.message_checks.load(Ordering::Acquire), 2);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert!(session.control.take_observation().is_none());
    assert!(!lock(&rig.client.core.state).stopping);
    Ok(())
}

#[test]
fn stopped_child_cannot_export_capability_and_clears_shared_secrets_before_late_retirement()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, false)?;
    connect(&rig, &session)?;
    assert!(lock(&session.control.inner.shared.state).capability.is_some());
    session.control.close();
    boundary.retiring.wait()?;
    assert_eq!(session.control.take_client_capability().err(), Some(LifecyclePipeError::Closed));
    let state = lock(&session.control.inner.shared.state);
    assert!(state.capability.is_none());
    assert!(state.authenticate.is_none());
    drop(state);
    assert!(session.completion.try_take_completion().is_none());
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Handshake)?;
    assert!(matches!(
        super::failures::completion(&rig, &mut session)?.outcome,
        JobOutcome::Cancelled
    ));
    Ok(())
}
