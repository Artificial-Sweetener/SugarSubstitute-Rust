// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Actual stopped transport/held-frame and fresh ancestor admission regressions.
use super::*;
use std::error::Error;
fn authenticate(rig: &Rig, session: &LifecyclePipeSession) -> TestResult {
    connect(rig, session)?;
    session.control.verify_capability([7; 32], Moment::from_nanos(1000))?;
    fact(rig, &session.control, |fact| matches!(fact, LifecyclePipeObservation::Authenticated(_)))?;
    Ok(())
}
#[test]
fn stopped_native_transport_rejects_application_admission_before_held_frame_allows_terminal()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    authenticate(&rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    let received = fact(&rig, &session.control, |fact| {
        matches!(fact, LifecyclePipeObservation::Received { .. })
    })?;
    let LifecyclePipeObservation::Received { frame, .. } = received else {
        return Err("Actual frame missing".into());
    };
    assert!(session.control.validate_current_admission().is_ok());
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert!(session.control.validate_current_admission().is_err());
    assert_eq!(frame.as_bytes(), b"Hello");
    drop(frame);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn original_clock_callback_closes_actual_ancestor_before_current_transport_fence() -> TestResult {
    let rig = Rig::new()?;
    let ancestor = scope(&rig)?;
    let child = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        Some(&ancestor),
        true,
    )?;
    let boundary = Boundary::new();
    let session = session(&rig, &child, &boundary, true)?;
    authenticate(&rig, &session)?;
    let core = rig.client.core.clone();
    let id = ancestor.id();
    rig.clock.on_next_current_thread_observation(Box::new(move || core.close_scope(id)));
    let cause = session
        .control
        .validate_current_admission()
        .err()
        .ok_or("Closed ancestor remained eligible")?;
    assert!(matches!(
        cause.source().and_then(|source| source.downcast_ref::<AdmissionError>()),
        Some(AdmissionError::ScopeClosed { .. })
    ));
    assert!(
        !lock(&rig.client.core.state)
            .scopes
            .get(&child.id())
            .ok_or("Actual child missing")?
            .cancel
            .is_requested()
    );
    Ok(())
}
#[test]
fn authenticated_current_transport_remains_eligible_after_original_handshake_timer_clears()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let session = session(&rig, &scope, &boundary, true)?;
    assert!(session.control.validate_current_admission().is_err());
    authenticate(&rig, &session)?;
    rig.clock.advance(1001);
    assert_eq!(session.control.original_deadline(), Moment::from_nanos(1000));
    assert!(session.control.validate_current_admission().is_ok());
    Ok(())
}
