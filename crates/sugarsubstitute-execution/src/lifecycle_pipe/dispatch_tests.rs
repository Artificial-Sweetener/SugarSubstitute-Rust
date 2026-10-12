// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe exact native issue facts and veto ordering through the real transport owner.
use super::*;
fn output(control: &LifecyclePipeControl) -> TestResult<LifecyclePipeWriteBuffer> {
    let mut output = control.take_write_buffer()?.ok_or("Missing output")?;
    output.as_mut_bytes()[..5].copy_from_slice(b"hello");
    Ok(output)
}
#[test]
fn veto_first_returns_exact_allocation_disarms_write_and_keeps_channel_usable() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.hold_write.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let mut buffer = output(&session.control)?;
    let address = buffer.as_mut_bytes().as_ptr();
    let admission = session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(100), buffer, 5)
        .map_err(|e| e.error)?;
    assert_eq!(admission.operation().get(), 1);
    boundary.preparing_write.wait()?;
    assert!(session.control.take_observation().is_none());
    assert_eq!(lock(&session.control.shared.state).write_slots, 2);
    assert_eq!(admission.revoke(), NativeWriteRevocation::RevokedBeforeIssue);
    assert_eq!(admission.revoke(), NativeWriteRevocation::AlreadyRevoked);
    rig.clock.advance(101);
    boundary.write_release.open();
    let LifecyclePipeObservation::WriteRevoked { operation, mut buffer } =
        fact(&rig, &session.control, |f| {
            matches!(f, LifecyclePipeObservation::WriteRevoked { .. })
        })?
    else {
        return Err("Missing veto terminal".into());
    };
    assert_eq!(operation.get(), 1);
    assert_eq!(buffer.as_mut_bytes().as_ptr(), address);
    assert_eq!(&buffer.as_mut_bytes()[..5], b"hello");
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.write_quiesced.load(Ordering::Acquire), 1);
    assert_eq!(lock(&session.control.shared.state).write_slots, 0);
    boundary.hold_write.store(false, Ordering::Release);
    let next = session
        .control
        .write(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900), buffer, 5)
        .map_err(|e| e.error)?;
    assert_eq!(admission.revoke(), NativeWriteRevocation::Retired);
    let issued = fact(&rig, &session.control, |f| {
        matches!(f, LifecyclePipeObservation::WriteIssued { .. })
    })?;
    assert!(
        matches!(issued, LifecyclePipeObservation::WriteIssued { at, .. } if at == Moment::from_nanos(101))
    );
    fact(&rig, &session.control, |f| {
        matches!(f, LifecyclePipeObservation::Written { bytes: 5, .. })
    })?;
    assert_eq!(next.revoke(), NativeWriteRevocation::AlreadyIssued { at: Moment::from_nanos(101) });
    session.control.close();
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn issue_first_revocation_preserves_pending_io_original_time_and_actual_terminal() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.hold_write.store(true, Ordering::Release);
    boundary.pending_write.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let admission = session
        .control
        .write(
            LifecyclePipeOperationId::new(1)?,
            Moment::from_nanos(300),
            output(&session.control)?,
            5,
        )
        .map_err(|e| e.error)?;
    boundary.preparing_write.wait()?;
    rig.clock.advance(75);
    boundary.write_release.open();
    boundary.writing.wait()?;
    let LifecyclePipeObservation::WriteIssued { operation, at } =
        session.control.take_observation().ok_or("Missing native acceptance")?
    else {
        return Err("Wrong fact".into());
    };
    assert_eq!(operation.get(), 1);
    assert_eq!(at, Moment::from_nanos(75));
    assert_eq!(admission.revoke(), NativeWriteRevocation::AlreadyIssued { at });
    assert_eq!(admission.revoke(), NativeWriteRevocation::AlreadyIssued { at });
    assert_eq!(lock(&session.control.shared.state).write_slots, 1);
    assert!(session.control.take_write_buffer()?.is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.write_quiesced.load(Ordering::Acquire), 0);
    rig.clock.wait_until_registered(Moment::from_nanos(300))?;
    rig.clock.advance(100);
    boundary.wake(Role::Write)?;
    fact(&rig, &session.control, |f| {
        matches!(f, LifecyclePipeObservation::Written { bytes: 5, .. })
    })?;
    assert_eq!(boundary.write_polls.load(Ordering::Acquire), 1);
    assert_eq!(boundary.write_quiesced.load(Ordering::Acquire), 1);
    assert_eq!(admission.revoke(), NativeWriteRevocation::AlreadyIssued { at });
    session.control.close();
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn failure_after_native_acceptance_cannot_erase_issued_history() -> TestResult {
    for after in [false, true] {
        let rig = Rig::new()?;
        let scope = scope(&rig)?;
        let boundary = Boundary::new();
        if after {
            boundary.fail_write_completion.store(true, Ordering::Release);
        } else {
            boundary.fail_write_issue.store(true, Ordering::Release);
        }
        let mut session = session(&rig, &scope, &boundary, true)?;
        connect(&rig, &session)?;
        let admission = session
            .control
            .write(
                LifecyclePipeOperationId::new(1)?,
                Moment::from_nanos(900),
                output(&session.control)?,
                5,
            )
            .map_err(|e| e.error)?;
        let JobOutcome::Failure(error) = failures::completion(&rig, &mut session)?.outcome else {
            return Err("Missing failure".into());
        };
        assert_eq!(
            error.native_cause().stage(),
            if after { "write_completion_rejected" } else { "write_issue_rejected" }
        );
        if after {
            assert!(matches!(
                session.control.take_observation(),
                Some(LifecyclePipeObservation::WriteIssued { .. })
            ));
            assert_eq!(
                admission.revoke(),
                NativeWriteRevocation::AlreadyIssued { at: Moment::from_nanos(0) }
            );
        } else {
            assert_eq!(admission.revoke(), NativeWriteRevocation::Retired);
        }
        assert!(session.control.take_observation().is_none());
    }
    Ok(())
}
#[test]
fn early_read_callback_keeps_issued_fact_before_received_and_never_retimestamps() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(true, Ordering::Release);
    boundary.pending_write.store(true, Ordering::Release);
    boundary.early_read.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::ReadPending { .. }))?;
    rig.clock.advance(23);
    let admission = session
        .control
        .write(
            LifecyclePipeOperationId::new(2)?,
            Moment::from_nanos(900),
            output(&session.control)?,
            5,
        )
        .map_err(|e| e.error)?;
    let issued = fact(&rig, &session.control, |f| {
        matches!(f, LifecyclePipeObservation::WriteIssued { .. })
    })?;
    assert!(
        matches!(issued, LifecyclePipeObservation::WriteIssued { at, .. } if at == Moment::from_nanos(23))
    );
    let received =
        fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Received { .. }))?;
    assert!(
        matches!(&received, LifecyclePipeObservation::Received { operation, frame } if operation.get() == 1 && frame.as_bytes() == b"Hello")
    );
    rig.clock.advance(17);
    assert_eq!(
        admission.revoke(),
        NativeWriteRevocation::AlreadyIssued { at: Moment::from_nanos(23) }
    );
    drop(received);
    session.control.close();
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn two_slot_preflight_rejects_without_transferring_the_original_buffer() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let mut buffer = output(&session.control)?;
    let address = buffer.as_mut_bytes().as_ptr();
    {
        let mut state = lock(&session.control.shared.state);
        for _ in 0..7 {
            state.observations.push_back(LifecyclePipeObservation::Prepared);
        }
    }
    let mut rejected = session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), buffer, 5)
        .err()
        .ok_or("Expected slot rejection")?;
    assert_eq!(rejected.error, LifecyclePipeError::Busy);
    assert_eq!(rejected.buffer.as_mut_bytes().as_ptr(), address);
    assert_eq!(&rejected.buffer.as_mut_bytes()[..5], b"hello");
    assert_eq!(lock(&session.control.shared.state).write_slots, 0);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    while session.control.take_observation().is_some() {}
    drop(rejected);
    session.control.close();
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
