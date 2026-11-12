// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive typed helper ownership through real admission and controlled native boundaries.
use super::*;
use crate::purpose_pipe::{
    Direction, Purpose, PurposeKey, PurposePipeControl, PurposePipeObservation as Fact,
    PurposePipeReport, PurposePipeSession, Side,
};
use sugarsubstitute_contracts::{
    incident::{DiagnosticKey, DiagnosticRole, DiagnosticSessionId},
    update::attempt::codec::{self, UpdateAttemptChannel, UpdateAttemptKey},
};
fn key() -> TestResult<DiagnosticKey> {
    Ok(DiagnosticKey::new(run()?.owner, DiagnosticRole::Storage, DiagnosticSessionId::new(1)?))
}
fn attempt() -> TestResult<UpdateAttemptChannel> {
    let attempt = std::num::NonZeroU64::new(4).ok_or("Zero attempt")?;
    Ok(UpdateAttemptChannel::new(run()?.owner, UpdateAttemptKey::new(attempt)))
}
fn session(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
    server: bool,
) -> TestResult<PurposePipeSession> {
    purpose_session(rig, scope, boundary, server, PurposeKey::Diagnostic(key()?))
}
fn purpose_session(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
    server: bool,
    key: PurposeKey,
) -> TestResult<PurposePipeSession> {
    *lock(&boundary.binding) = Some(PeerBinding::Purpose(key));
    let mut job = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    job.operation = Operation::NativeLifecycle;
    let (inner, completion) = admit(
        &rig.client.core,
        scope,
        job,
        server.then_some(PeerBinding::Purpose(key)),
        Moment::from_nanos(1000),
        Box::new(Lease(boundary.clone())),
        Owner::Purpose,
    )?;
    let side = if server { Side::Parent } else { Side::Helper };
    let purpose = key.purpose();
    crate::purpose_pipe::apply_loan_rule(&inner, purpose);
    Ok(PurposePipeSession { control: PurposePipeControl { inner, purpose, side }, completion })
}
fn fact(
    rig: &Rig,
    session: &mut PurposePipeSession,
    select: impl Fn(&Fact) -> bool,
) -> TestResult<Fact> {
    loop {
        let observed = rig.wake.count();
        for value in session.drain(NonZeroUsize::MIN) {
            if select(&value) {
                return Ok(value);
            }
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        for value in session.drain(NonZeroUsize::MIN) {
            if select(&value) {
                return Ok(value);
            }
        }
        rig.wake.wait_after(observed)?;
    }
}
fn terminal(
    rig: &Rig,
    session: &mut PurposePipeSession,
) -> TestResult<JobCompletion<PurposePipeReport, LifecyclePipeFailure>> {
    loop {
        let observed = rig.wake.count();
        if rig.client.drain_completions(NonZeroUsize::MIN).is_err() {
            continue;
        }
        if let Some(value) = session.take_terminal() {
            return Ok(value);
        }
        rig.wake.wait_after(observed)?;
    }
}
fn connected(rig: &Rig, session: &mut PurposePipeSession) -> TestResult {
    session.control.inner.bind_input(PeerInput::Simulated);
    fact(rig, session, |f| matches!(f, Fact::Connected))?;
    Ok(())
}
fn authenticate(rig: &Rig, session: &mut PurposePipeSession) -> TestResult {
    authenticate_as(rig, session, PurposeKey::Diagnostic(key()?))
}
fn authenticate_as(rig: &Rig, session: &mut PurposePipeSession, key: PurposeKey) -> TestResult {
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    drop(fact(rig, session, |f| matches!(f, Fact::Received { .. }))?);
    session.control.verify_capability([7; 32], Moment::from_nanos(900))?;
    let Fact::Authenticated(peer) = fact(rig, session, |f| matches!(f, Fact::Authenticated(_)))?
    else {
        return Err("Missing peer".into());
    };
    assert_eq!(peer.key(), key);
    assert_eq!((peer.process_id(), peer.creation_time_100ns(), peer.session_id()), (71, 99, 1));
    Ok(())
}
#[test]
fn helper_binding_and_authenticated_passive_receive_survive_original_handshake() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    authenticate(&rig, &mut session)?;
    boundary.pending_read.store(true, Ordering::Release);
    session.control.read_passive(LifecyclePipeOperationId::new(2)?)?;
    fact(&rig, &mut session, |f| matches!(f, Fact::ReadPending { .. }))?;
    rig.clock.advance(2000);
    assert_eq!(session.control.prepared_key(), Some(PurposeKey::Diagnostic(key()?)));
    assert!(session.take_terminal().is_none());
    session.control.close();
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn update_attempt_parent_sends_only_its_cancel_request_and_the_helper_writes_freely() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(false, Ordering::Release);
    let channel = PurposeKey::UpdateAttempt(attempt()?);
    let mut parent = purpose_session(&rig, &scope, &boundary, true, channel)?;
    assert_eq!(parent.control.purpose(), Purpose::UpdateAttempt);
    assert_eq!(Purpose::UpdateAttempt.direction(), Direction::HelperReports);
    connected(&rig, &mut parent)?;
    authenticate_as(&rig, &mut parent, channel)?;
    let output = parent.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    let rejected = parent
        .control
        .write(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900), output, 1)
        .err()
        .ok_or("Parent wrote arbitrary bytes on a helper-reporting channel")?;
    assert_eq!(rejected.error, LifecyclePipeError::Direction);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    parent
        .control
        .write_cancel_request(
            LifecyclePipeOperationId::new(2)?,
            Moment::from_nanos(900),
            rejected.buffer,
        )
        .map_err(|rejection| rejection.error)?;
    let Fact::Written { bytes, .. } =
        fact(&rig, &mut parent, |f| matches!(f, Fact::Written { .. }))?
    else {
        return Err("Missing cancel write".into());
    };
    assert_eq!(bytes as usize, codec::encode_cancel(attempt()?.attempt())?.len());
    parent.control.close();
    drop(terminal(&rig, &mut parent)?);
    Ok(())
}
#[test]
fn update_attempt_helper_cannot_send_the_parents_cancel_request() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let channel = PurposeKey::UpdateAttempt(attempt()?);
    let mut helper = purpose_session(&rig, &scope, &boundary, false, channel)?;
    assert_eq!(helper.control.purpose(), Purpose::UpdateAttempt);
    connected(&rig, &mut helper)?;
    let output = helper.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    let rejected = helper
        .control
        .write_cancel_request(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output)
        .err()
        .ok_or("Helper sent the parent's cancel request")?;
    assert_eq!(rejected.error, LifecyclePipeError::Direction);
    // Closing while the caller still holds the rejected buffer must settle, not wait on it.
    helper.control.close();
    drop(terminal(&rig, &mut helper)?);
    drop(rejected);
    Ok(())
}
#[test]
fn update_attempt_parent_holding_a_received_frame_still_settles_on_close() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(false, Ordering::Release);
    let channel = PurposeKey::UpdateAttempt(attempt()?);
    let mut parent = purpose_session(&rig, &scope, &boundary, true, channel)?;
    connected(&rig, &mut parent)?;
    authenticate_as(&rig, &mut parent, channel)?;
    parent.control.read(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900))?;
    let frame = fact(&rig, &mut parent, |f| matches!(f, Fact::Received { .. }))?;
    let output = parent.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    parent.control.close();
    assert!(matches!(terminal(&rig, &mut parent)?.outcome, JobOutcome::Cancelled));
    // Each held guard still carries its own charge after settlement.
    assert_eq!(rig.client.core.scope_snapshot(scope.id()).reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    drop((frame, output));
    Ok(())
}
#[test]
fn diagnostic_loans_still_fence_retirement_for_their_ordering_owners() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.take_terminal().is_none());
    drop(output);
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn diagnostic_parent_keeps_two_way_writes() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    assert_eq!(session.control.purpose().direction(), Direction::Both);
    connected(&rig, &mut session)?;
    authenticate(&rig, &mut session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    let rejected = session
        .control
        .write_cancel_request(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900), output)
        .err()
        .ok_or("Diagnostic channel sent an update cancel request")?;
    assert_eq!(rejected.error, LifecyclePipeError::Direction);
    session
        .control
        .write(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900), rejected.buffer, 1)
        .map_err(|rejection| rejection.error)?;
    fact(&rig, &mut session, |f| matches!(f, Fact::Written { .. }))?;
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}
#[test]
fn bootstrap_and_enrollment_transfer_atomically_and_preserve_exact_session_pin() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    fact(&rig, &mut session, |f| matches!(f, Fact::Prepared))?;
    let (bootstrap, mut enrollment) =
        crate::lifecycle_pipe::enrollment::Enrollment::take_bootstrap(&session.control.inner)?
            .ok_or("Missing launch pins")?;
    assert!(enrollment.matches(&bootstrap.shared));
    assert!(matches!(
        crate::lifecycle_pipe::enrollment::Enrollment::take_bootstrap(&session.control.inner),
        Err(LifecyclePipeError::AlreadyTransferred)
    ));
    let other = DiagnosticKey::new(key()?.owner(), DiagnosticRole::Reporter, key()?.session());
    let process = ProcessKey { owner: key()?.owner(), id: ProcessId::new(1)? };
    assert!(matches!(
        enrollment.bind(PeerBinding::Purpose(PurposeKey::Diagnostic(other)), process, 19, |_| {
            PeerInput::Simulated
        }),
        Err((LifecyclePipeError::Closed, 19))
    ));
    enrollment
        .bind(PeerBinding::Purpose(PurposeKey::Diagnostic(key()?)), process, (), |_| {
            PeerInput::Simulated
        })
        .map_err(|(e, _)| e)?;
    fact(&rig, &mut session, |f| matches!(f, Fact::Connected))?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.take_terminal().is_none());
    drop(bootstrap);
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn diagnostic_close_fences_connect_and_read_after_inert_native_preparation() -> TestResult {
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
            boundary.preparing_connect.wait()?;
        }
        session.control.close();
        if read {
            boundary.read_release.open()
        } else {
            boundary.connect_release.open()
        }
        assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
        assert_eq!(boundary.message_checks.load(Ordering::Acquire), 0);
        assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    }
    Ok(())
}
#[path = "diagnostic_custody_tests.rs"]
mod custody;
#[path = "diagnostic_retirement_tests.rs"]
mod retirement;
