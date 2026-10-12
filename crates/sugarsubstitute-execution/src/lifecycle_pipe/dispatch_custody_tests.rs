// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve native and original allocation custody through revocation and owner faults.
use super::*;
#[test]
fn native_panic_reports_uncertain_issue_and_retains_claim_until_actual_retirement() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.panic_write.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing buffer")?;
    let admission = session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|e| e.error)?;
    boundary.retiring.wait()?;
    assert_eq!(
        admission.revoke(),
        NativeWriteRevocation::IssueUncertain { at: Moment::from_nanos(0) }
    );
    assert!(session.control.take_observation().is_none());
    assert!(session.completion.try_take_completion().is_none());
    let memory = lock(&boundary.memory).take().ok_or("Missing memory lease")?;
    assert!(memory.claim(1).is_err());
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Write)?;
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::Panicked(_)));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn consumed_revoked_buffer_blocks_terminal_but_unconsumed_buffer_is_abandoned() -> TestResult {
    for consume in [false, true] {
        let rig = Rig::new()?;
        let scope = scope(&rig)?;
        let boundary = Boundary::new();
        boundary.hold_write.store(true, Ordering::Release);
        let mut session = session(&rig, &scope, &boundary, true)?;
        connect(&rig, &session)?;
        let output = session.control.take_write_buffer()?.ok_or("Missing buffer")?;
        let admission = session
            .control
            .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
            .map_err(|e| e.error)?;
        boundary.preparing_write.wait()?;
        assert_eq!(admission.revoke(), NativeWriteRevocation::RevokedBeforeIssue);
        boundary.write_release.open();
        let held = if consume {
            Some(fact(&rig, &session.control, |f| {
                matches!(f, LifecyclePipeObservation::WriteRevoked { .. })
            })?)
        } else {
            loop {
                let observed = rig.wake.count();
                if lock(&session.control.shared.state)
                    .observations
                    .iter()
                    .any(|f| matches!(f, LifecyclePipeObservation::WriteRevoked { .. }))
                {
                    break;
                }
                rig.client.drain_completions(NonZeroUsize::MIN)?;
                if lock(&session.control.shared.state)
                    .observations
                    .iter()
                    .any(|f| matches!(f, LifecyclePipeObservation::WriteRevoked { .. }))
                {
                    break;
                }
                rig.wake.wait_after(observed)?;
            }
            None
        };
        session.control.close();
        boundary.retiring.wait()?;
        if consume {
            assert!(session.completion.try_take_completion().is_none());
            assert!(scope.snapshot().reserved_bytes >= MIN_LIFECYCLE_PIPE_BYTES);
        }
        drop(held);
        assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
        assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
        assert!(session.control.take_observation().is_none());
    }
    Ok(())
}

#[test]
fn vetoed_prepared_callback_failure_retains_original_storage_through_failed_retirement()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.hold_write.store(true, Ordering::Release);
    boundary.fail_write_quiesce.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing buffer")?;
    let admission = session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|e| e.error)?;
    boundary.preparing_write.wait()?;
    assert_eq!(admission.revoke(), NativeWriteRevocation::RevokedBeforeIssue);
    boundary.write_release.open();
    boundary.retiring.wait()?;
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(session.control.take_observation().is_none());
    assert!(session.completion.try_take_completion().is_none());
    let memory = lock(&boundary.memory).take().ok_or("Missing memory")?;
    assert!(memory.claim(1).is_err());
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Write)?;
    let JobOutcome::Failure(error) = failures::completion(&rig, &mut session)?.outcome else {
        return Err("Expected callback failure".into());
    };
    assert_eq!(error.native_cause().stage(), "write_callback_quiesce");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
