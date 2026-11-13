// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact charged custody, failure causes and bounded diagnostic observations.
use super::*;
#[test]
fn diagnostic_rejected_write_retains_actual_original_guard_provenance() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    let mut guard = session.control.take_write_buffer()?.ok_or("Missing guard")?;
    guard.as_mut_bytes()[..4].copy_from_slice(b"Data");
    let mut rejected = session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), guard, 0)
        .err()
        .ok_or("Accepted empty write")?;
    assert_eq!(rejected.error, LifecyclePipeError::InvalidMessage);
    assert_eq!(&rejected.buffer.as_mut_bytes()[..4], b"Data");
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.take_terminal().is_none());
    drop(rejected);
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn diagnostic_frame_and_partial_native_retirement_retain_actual_charge() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    let frame = fact(&rig, &mut session, |f| matches!(f, Fact::Received { .. }))?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.take_terminal().is_none());
    assert_eq!(rig.client.core.scope_snapshot(scope.id()).reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Read)?;
    assert!(session.take_terminal().is_none());
    drop(frame);
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn diagnostic_mismatched_actual_peer_binding_fails_before_authentication_fact() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    drop(fact(&rig, &mut session, |f| matches!(f, Fact::Received { .. }))?);
    *lock(&boundary.authenticated_binding) = Some(PeerBinding::Desktop(run()?));
    session.control.verify_capability([7; 32], Moment::from_nanos(900))?;
    let JobOutcome::Failure(error) = terminal(&rig, &mut session)?.outcome else {
        return Err("Missing failure".into());
    };
    assert_eq!(error.native_cause().stage(), "pipe_authenticated_binding");
    assert!(
        !session
            .drain(NonZeroUsize::new(32).ok_or("Invalid bound")?)
            .into_iter()
            .any(|f| matches!(f, Fact::Authenticated(_)))
    );
    Ok(())
}
#[test]
fn diagnostic_partial_preparation_failure_preserves_typed_cause_and_retirement() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    boundary.retiring.wait()?;
    assert!(session.take_terminal().is_none());
    assert_eq!(rig.client.core.scope_snapshot(scope.id()).reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Read)?;
    let JobOutcome::Failure(error) = terminal(&rig, &mut session)?.outcome else {
        return Err("Missing failure".into());
    };
    assert_eq!(error.native_cause().stage(), "partial_pipe_creation");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn diagnostic_fixed_eight_observation_slots_reject_overload_without_losing_guard() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    for id in 1..=4 {
        let mut buffer = session.control.take_write_buffer()?.ok_or("Missing output")?;
        buffer.as_mut_bytes()[0] = id as u8;
        session
            .control
            .write(LifecyclePipeOperationId::new(id)?, Moment::from_nanos(900), buffer, 1)
            .map_err(|e| e.error)?;
        loop {
            let observed = rig.wake.count();
            if lock(&session.control.inner.shared.state).written == id {
                break;
            }
            rig.client.drain_completions(NonZeroUsize::MIN)?;
            if lock(&session.control.inner.shared.state).written == id {
                break;
            }
            rig.wake.wait_after(observed)?;
        }
    }
    let mut buffer = session.control.take_write_buffer()?.ok_or("Missing output")?;
    buffer.as_mut_bytes()[0] = 99;
    let mut rejection = session
        .control
        .write(LifecyclePipeOperationId::new(5)?, Moment::from_nanos(900), buffer, 1)
        .err()
        .ok_or("Accepted overload")?;
    assert_eq!(rejection.error, LifecyclePipeError::Busy);
    assert_eq!(rejection.buffer.as_mut_bytes()[0], 99);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 4);
    let facts = session.drain(NonZeroUsize::new(32).ok_or("Invalid drain")?);
    assert_eq!(facts.len(), 8);
    assert!(!facts.is_empty());
    assert_eq!(facts.into_iter().count(), 8);
    assert!(session.drain(NonZeroUsize::MIN).is_empty());
    drop(rejection);
    session.control.close();
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn diagnostic_scope_cancel_during_inert_connect_preparation_preserves_retirement() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.hold_connect.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, false)?;
    session.control.inner.bind_input(PeerInput::Simulated);
    boundary.preparing_connect.wait()?;
    scope.cancel();
    boundary.connect_release.open();
    boundary.retiring.wait()?;
    assert_eq!(boundary.adopted.load(Ordering::Acquire), 1);
    assert!(session.take_terminal().is_none());
    assert_eq!(rig.client.core.scope_snapshot(scope.id()).reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Handshake)?;
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn diagnostic_original_deadline_expiry_fences_connect_and_read_without_renewal() -> TestResult {
    for read in [false, true] {
        let rig = Rig::new()?;
        let scope = scope(&rig)?;
        let boundary = Boundary::new();
        if !read {
            boundary.hold_connect.store(true, Ordering::Release)
        }
        let mut session = session(&rig, &scope, &boundary, false)?;
        session.control.inner.bind_input(PeerInput::Simulated);
        if read {
            fact(&rig, &mut session, |f| matches!(f, Fact::Connected))?;
            boundary.hold_read.store(true, Ordering::Release);
            session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
            boundary.preparing_read.wait()?;
        } else {
            boundary.preparing_connect.wait()?
        }
        rig.clock.advance(1000);
        if read {
            boundary.read_release.open()
        } else {
            boundary.connect_release.open()
        }
        assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::TimedOut));
        assert_eq!(boundary.message_checks.load(Ordering::Acquire), 0);
    }
    Ok(())
}
#[test]
fn diagnostic_fatal_message_identity_failure_stops_admission_and_retains_original_cause()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.allow_retire.store(false, Ordering::Release);
    boundary.fatal_message.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    boundary.retiring.wait()?;
    assert!(session.take_terminal().is_none());
    assert!(boundary.restoration_failed.load(Ordering::Acquire));
    assert!(rig.client.core.native_identity_unrestored.load(Ordering::Acquire));
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Read)?;
    let JobOutcome::Failure(error) = terminal(&rig, &mut session)?.outcome else {
        return Err("Missing fatal failure".into());
    };
    assert!(error.requires_process_shutdown());
    assert_eq!(error.native_cause().stage(), "failed_identity_restore");
    Ok(())
}
#[test]
fn diagnostic_invalid_capability_never_publishes_authenticated_fact() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    drop(fact(&rig, &mut session, |f| matches!(f, Fact::Received { .. }))?);
    session.control.verify_capability([8; 32], Moment::from_nanos(900))?;
    let JobOutcome::Failure(error) = terminal(&rig, &mut session)?.outcome else {
        return Err("Missing authentication failure".into());
    };
    assert_eq!(error.native_cause().stage(), "fake_credential_rejection");
    assert!(
        !session
            .drain(NonZeroUsize::new(32).ok_or("Invalid drain")?)
            .into_iter()
            .any(|f| matches!(f, Fact::Authenticated(_)))
    );
    Ok(())
}
