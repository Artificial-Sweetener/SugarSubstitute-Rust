// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise authenticated passive reads through real admission, native worker and retirement.
use super::*;

struct RetirementHold(Arc<Boundary>);
impl RetirementHold {
    fn new(boundary: &Arc<Boundary>) -> Self {
        boundary.allow_retire.store(false, Ordering::Release);
        Self(boundary.clone())
    }
}
impl Drop for RetirementHold {
    fn drop(&mut self) {
        self.0.allow_retire.store(true, Ordering::Release);
        if let Some(signals) = lock(&self.0.signals).as_ref() {
            signals[Role::Read.index()].complete();
        }
    }
}

fn authenticated(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
) -> TestResult<LifecyclePipeSession> {
    let session = session(rig, scope, boundary, true)?;
    connect(rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    let received =
        fact(rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Received { .. }))?;
    drop(received);
    session.control.verify_capability([7; 32], Moment::from_nanos(1000))?;
    fact(rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Authenticated(_)))?;
    Ok(session)
}

fn passive_pending(rig: &Rig, session: &LifecyclePipeSession, boundary: &Boundary) -> TestResult {
    boundary.pending_read.store(true, Ordering::Release);
    session.control.read_passive(LifecyclePipeOperationId::new(2)?)?;
    fact(
        rig,
        &session.control,
        |f| matches!(f, LifecyclePipeObservation::ReadPending { operation } if operation.get() == 2),
    )?;
    Ok(())
}

#[test]
fn unauthenticated_passive_rejection_preserves_identity_storage_and_finite_read_expiry()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    assert_eq!(
        session.control.read_passive(LifecyclePipeOperationId::new(1)?),
        Err(LifecyclePipeError::NotReady)
    );
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(400))?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::ReadPending { .. }))?;
    rig.clock.wait_until_registered(Moment::from_nanos(400))?;
    assert_eq!(session.control.original_deadline(), Moment::from_nanos(1000));
    rig.clock.advance(400);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn unauthenticated_passive_rejection_keeps_original_handshake_timer_armed() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    assert_eq!(
        session.control.read_passive(LifecyclePipeOperationId::new(1)?),
        Err(LifecyclePipeError::NotReady)
    );
    rig.clock.wait_until_registered(Moment::from_nanos(1000))?;
    rig.clock.advance(1000);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.message_checks.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn authenticated_idle_passive_receive_stays_live_and_transfers_actual_verified_bytes() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = authenticated(&rig, &scope, &boundary)?;
    passive_pending(&rig, &session, &boundary)?;
    rig.clock.advance(6_000_000_000);
    let mut output = session.control.take_write_buffer()?.ok_or("Missing output")?;
    output.as_mut_bytes()[..4].copy_from_slice(b"Ping");
    session
        .control
        .write(LifecyclePipeOperationId::new(3)?, Moment::from_nanos(7_000_000_000), output, 4)
        .map_err(|e| e.error)?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Written { .. }))?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert_eq!(session.control.original_deadline(), Moment::from_nanos(1000));
    boundary.wake(Role::Read)?;
    let received =
        fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Received { .. }))?;
    let LifecyclePipeObservation::Received { operation, frame } = received else {
        return Err("Wrong observation".into());
    };
    assert_eq!(operation.get(), 2);
    assert_eq!(frame.as_bytes(), b"Hello");
    assert_eq!(boundary.message_checks.load(Ordering::Acquire), 2);
    drop(frame);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn passive_received_guard_blocks_reuse_and_retains_charge_through_native_close() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = authenticated(&rig, &scope, &boundary)?;
    session.control.read_passive(LifecyclePipeOperationId::new(2)?)?;
    let received =
        fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Received { .. }))?;
    assert_eq!(
        session.control.read_passive(LifecyclePipeOperationId::new(2)?),
        Err(LifecyclePipeError::InvalidOperation)
    );
    assert_eq!(
        session.control.read_passive(LifecyclePipeOperationId::new(3)?),
        Err(LifecyclePipeError::Busy)
    );
    drop(received);
    session.control.read_passive(LifecyclePipeOperationId::new(3)?)?;
    let received = fact(
        &rig,
        &session.control,
        |f| matches!(f, LifecyclePipeObservation::Received { operation, .. } if operation.get() == 3),
    )?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    drop(received);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn passive_control_and_watchdog_scope_cancellation_wait_for_checked_native_retirement() -> TestResult
{
    for watchdog_scope in [false, true] {
        let rig = Rig::new()?;
        let scope = scope(&rig)?;
        let boundary = Boundary::new();
        let mut session = authenticated(&rig, &scope, &boundary)?;
        passive_pending(&rig, &session, &boundary)?;
        let hold = RetirementHold::new(&boundary);
        if watchdog_scope {
            rig.clock.advance(5_000_000_000);
            scope.cancel();
        } else {
            session.control.close();
        }
        boundary.retiring.wait()?;
        assert!(session.completion.try_take_completion().is_none());
        assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
        assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
        drop(hold);
        assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
        assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    }
    Ok(())
}

#[test]
fn passive_peer_exit_preserves_actual_error_and_custody_until_checked_close() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = authenticated(&rig, &scope, &boundary)?;
    passive_pending(&rig, &session, &boundary)?;
    let hold = RetirementHold::new(&boundary);
    boundary.peer_closed.store(true, Ordering::Release);
    boundary.wake(Role::Read)?;
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    drop(hold);
    let terminal = rig.completion(&mut session.completion)?;
    let JobOutcome::Failure(cause) = terminal.outcome else {
        return Err("Missing actual peer failure".into());
    };
    assert_eq!(cause.native_cause().stage(), "passive_peer_closed");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn original_finite_write_expiry_still_retires_pending_passive_receive() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = authenticated(&rig, &scope, &boundary)?;
    passive_pending(&rig, &session, &boundary)?;
    boundary.pending_write.store(true, Ordering::Release);
    let hold = RetirementHold::new(&boundary);
    let mut output = session.control.take_write_buffer()?.ok_or("Missing output")?;
    output.as_mut_bytes()[..4].copy_from_slice(b"Ping");
    session
        .control
        .write(LifecyclePipeOperationId::new(3)?, Moment::from_nanos(400), output, 4)
        .map_err(|e| e.error)?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::WriteIssued { .. }))?;
    rig.clock.wait_until_registered(Moment::from_nanos(400))?;
    rig.clock.advance(400);
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    drop(hold);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
