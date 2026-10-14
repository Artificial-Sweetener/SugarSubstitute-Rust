// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive typed helper ownership through real admission and controlled native boundaries.
use super::*;
use crate::diagnostic_pipe::{
    DiagnosticPipeControl, DiagnosticPipeObservation as Fact, DiagnosticPipeReport,
    DiagnosticPipeSession,
};
use sugarsubstitute_contracts::incident::{DiagnosticKey, DiagnosticRole, DiagnosticSessionId};
fn key() -> TestResult<DiagnosticKey> {
    Ok(DiagnosticKey::new(run()?.owner, DiagnosticRole::Storage, DiagnosticSessionId::new(1)?))
}
fn session(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
    server: bool,
) -> TestResult<DiagnosticPipeSession> {
    let key = key()?;
    *lock(&boundary.binding) = Some(PeerBinding::Diagnostic(key));
    let mut job = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    job.operation = Operation::NativeLifecycle;
    let (inner, completion) = admit(
        &rig.client.core,
        scope,
        job,
        server.then_some(PeerBinding::Diagnostic(key)),
        Moment::from_nanos(1000),
        Box::new(Lease(boundary.clone())),
        Owner::Diagnostic,
    )?;
    Ok(DiagnosticPipeSession { control: DiagnosticPipeControl { inner }, completion })
}
fn fact(
    rig: &Rig,
    session: &mut DiagnosticPipeSession,
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
    session: &mut DiagnosticPipeSession,
) -> TestResult<JobCompletion<DiagnosticPipeReport, LifecyclePipeFailure>> {
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
fn connected(rig: &Rig, session: &mut DiagnosticPipeSession) -> TestResult {
    session.control.inner.bind_input(PeerInput::Simulated);
    fact(rig, session, |f| matches!(f, Fact::Connected))?;
    Ok(())
}
fn authenticate(rig: &Rig, session: &mut DiagnosticPipeSession) -> TestResult {
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    drop(fact(rig, session, |f| matches!(f, Fact::Received { .. }))?);
    session.control.verify_capability([7; 32], Moment::from_nanos(900))?;
    let Fact::Authenticated(peer) = fact(rig, session, |f| matches!(f, Fact::Authenticated(_)))?
    else {
        return Err("Missing peer".into());
    };
    assert_eq!(peer.key(), key()?);
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
    assert_eq!(session.control.prepared_key(), Some(key()?));
    assert!(session.take_terminal().is_none());
    session.control.close();
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
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
        enrollment.bind(PeerBinding::Diagnostic(other), process, 19, |_| PeerInput::Simulated),
        Err((LifecyclePipeError::Closed, 19))
    ));
    enrollment
        .bind(PeerBinding::Diagnostic(key()?), process, (), |_| PeerInput::Simulated)
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
