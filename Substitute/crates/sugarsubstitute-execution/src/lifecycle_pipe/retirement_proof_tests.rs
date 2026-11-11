// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require actual checked worker disposal independently of its original operation result.
use super::*;

#[test]
fn failed_operation_delivers_checked_retirement_after_actual_lease_disposal() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    boundary.retiring.wait()?;
    assert!(session.control.take_checked_retirement().is_none());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Handshake)?;
    let completed = failures::completion(&rig, &mut session)?;
    let JobOutcome::Failure(cause) = &completed.outcome else {
        return Err("Original native failure lost".into());
    };
    assert_eq!(cause.native_cause().stage(), "partial_pipe_creation");
    let proof = session.control.take_checked_retirement().ok_or("Missing checked retirement")?;
    assert!(proof.matches(&session.control));
    assert_eq!(proof.execution_id(), scope.owner_id());
    assert_eq!(proof.scope_id(), completed.scope_id);
    assert_eq!(proof.spec(), completed.spec);
    assert_eq!(proof.run(), Some(run()?));
    assert_eq!(proof.spec().deadline, None);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 1);
    assert!(session.control.take_checked_retirement().is_none());
    Ok(())
}

#[test]
fn failed_close_and_retirement_panic_retain_custody_without_checked_proof() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    boundary.panic_retire.store(true, Ordering::Release);
    boundary.retire_failures.store(1, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    boundary.close_failed.wait()?;
    assert!(session.control.take_checked_retirement().is_none());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    let memory = lock(&boundary.memory).take().ok_or("Missing actual retained claim")?;
    assert!(memory.claim(1).is_err());
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Handshake)?;
    let completed = failures::completion(&rig, &mut session)?;
    let JobOutcome::Failure(cause) = &completed.outcome else {
        return Err("Original native failure lost".into());
    };
    assert_eq!(cause.native_cause().stage(), "partial_pipe_creation");
    assert!(cause.contains_owner_panic());
    let proof = session.control.take_checked_retirement().ok_or("Missing final checked proof")?;
    assert!(proof.matches(&session.control));
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn checked_native_close_waits_for_actual_output_loan_before_proof() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing actual output")?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.control.take_checked_retirement().is_none());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 0);
    drop(output);
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    let proof = session.control.take_checked_retirement().ok_or("Missing returned-loan proof")?;
    assert!(proof.matches(&session.control));
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn checked_proof_rejects_foreign_actual_session_with_equal_numeric_facts() -> TestResult {
    let first = Rig::new()?;
    let first_scope = scope(&first)?;
    let boundary = Boundary::new();
    let mut original = session(&first, &first_scope, &boundary, true)?;
    connect(&first, &original)?;
    original.control.close();
    let first_completed = failures::completion(&first, &mut original)?;
    assert!(matches!(first_completed.outcome, JobOutcome::Cancelled));
    let proof = original.control.take_checked_retirement().ok_or("Missing original proof")?;
    let second = Rig::new()?;
    let second_scope = scope(&second)?;
    let other_boundary = Boundary::new();
    let mut other = session(&second, &second_scope, &other_boundary, true)?;
    connect(&second, &other)?;
    assert_eq!(original.completion.spec(), other.completion.spec());
    assert_eq!(original.completion.scope_id(), other.completion.scope_id());
    assert_eq!(proof.run(), other.control.prepared_run());
    assert!(!proof.matches(&other.control));
    other.control.close();
    let other_completed = failures::completion(&second, &mut other)?;
    assert!(matches!(other_completed.outcome, JobOutcome::Cancelled));
    assert!(proof.matches(&original.control));
    Ok(())
}

#[test]
fn actual_launch_pin_blocks_checked_retirement_until_its_return() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    fact(&rig, &session.control, |fact| matches!(fact, LifecyclePipeObservation::Prepared))?;
    let pin = session.control.take_bootstrap()?.ok_or("Missing actual bootstrap pin")?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.control.take_checked_retirement().is_none());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 0);
    drop(pin);
    let completed = failures::completion(&rig, &mut session)?;
    assert!(matches!(completed.outcome, JobOutcome::Cancelled));
    let proof = session.control.take_checked_retirement().ok_or("Missing returned-pin proof")?;
    assert!(proof.matches(&session.control));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 1);
    Ok(())
}
