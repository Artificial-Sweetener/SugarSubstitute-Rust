// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require the real diagnostic session's checked disposal and unique allocation.
use super::*;

#[test]
fn diagnostic_checked_retirement_waits_for_original_output_loan() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connected(&rig, &mut session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing output")?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.control.take_checked_retirement().is_none());
    assert!(session.take_terminal().is_none());
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 0);
    drop(output);
    let completion = terminal(&rig, &mut session)?;
    let proof = session.control.take_checked_retirement().ok_or("Missing checked proof")?;
    assert!(proof.matches_diagnostic(&session.control));
    assert_eq!(proof.execution_id(), scope.owner_id());
    assert_eq!(proof.scope_id(), completion.scope_id);
    assert_eq!(proof.spec(), completion.spec);
    assert_eq!(proof.run(), None);
    assert!(session.control.take_checked_retirement().is_none());
    Ok(())
}

#[test]
fn diagnostic_checked_retirement_rejects_equal_numeric_foreign_session() -> TestResult {
    let first = Rig::new()?;
    let first_scope = scope(&first)?;
    let mut original = session(&first, &first_scope, &Boundary::new(), true)?;
    connected(&first, &mut original)?;
    original.control.close();
    let completion = terminal(&first, &mut original)?;
    let proof = original.control.take_checked_retirement().ok_or("Missing checked proof")?;
    let second = Rig::new()?;
    let second_scope = scope(&second)?;
    let mut other = session(&second, &second_scope, &Boundary::new(), true)?;
    connected(&second, &mut other)?;
    assert_eq!(original.control.prepared_key(), other.control.prepared_key());
    assert_eq!(proof.scope_id(), other.completion.scope_id());
    assert_eq!(proof.spec(), other.completion.spec());
    assert!(!proof.matches_diagnostic(&other.control));
    assert!(proof.matches_diagnostic(&original.control));
    assert_eq!(proof.spec(), completion.spec);
    other.control.close();
    assert!(matches!(terminal(&second, &mut other)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
