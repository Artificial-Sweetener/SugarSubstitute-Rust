// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Extract only genuine recovery pipe retirement after actual native-boundary and loan disposal.
use super::*;
use sugarsubstitute_contracts::{incident::*, recovery_supervisor::*};
fn key() -> TestResult<RecoveryGrantKey> {
    Ok(RecoveryGrantKey::new(
        RecoveryGrantId::new(1)?,
        run()?,
        ReportBinding::new(
            IncidentKey::new(IncidentSubject::Desktop(run()?)),
            IncidentRevision::new(1)?,
            ReportSessionId::new(1)?,
        ),
    )?)
}
#[test]
fn recovery_checked_retirement_waits_for_original_output_loan_and_retains_exact_claim() -> TestResult
{
    let rig = Rig::new()?;
    let (scope, mut session, boundary) = recovery_child(&rig, 2, 1, key()?)?;
    let output = session.control.take_write_buffer()?.ok_or("Original output loan missing")?;
    session.control.close();
    boundary.0.retiring.wait()?;
    assert!(session.control.take_checked_retirement().is_none());
    assert!(session.take_terminal().is_none());
    assert_eq!(boundary.0.lease_drops.load(Ordering::Acquire), 0);
    drop(output);
    let completion = rig.completion(&mut session.completion)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    let proof = session
        .control
        .take_checked_retirement()
        .ok_or("Genuine checked recovery retirement missing")?;
    assert!(proof.matches(&session.control));
    assert_eq!(proof.key(), key()?);
    assert_eq!(proof.execution_id(), scope.owner_id());
    assert_eq!(proof.scope_id(), completion.scope_id);
    assert_eq!(proof.spec(), completion.spec);
    assert!(session.control.take_checked_retirement().is_none());
    assert_eq!(boundary.0.lease_drops.load(Ordering::Acquire), 1);
    let core = rig.client.core.clone();
    drop((completion, session, boundary));
    let Rig { owner, client, clock, wake, scope: ordinary } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    assert_eq!(lock(&core.state).bytes, MIN_LIFECYCLE_PIPE_BYTES);
    drop(proof);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(lock(&core.state).bytes, 0);
    drop((scope, ordinary, client, clock, wake));
    Ok(())
}
#[test]
fn recovery_checked_retirement_rejects_equal_key_and_numeric_foreign_allocation() -> TestResult {
    let first = Rig::new()?;
    let (first_scope, mut original, original_boundary) = recovery_child(&first, 2, 1, key()?)?;
    original.control.close();
    let completion = first.completion(&mut original.completion)?;
    let proof =
        original.control.take_checked_retirement().ok_or("Original recovery proof missing")?;
    let second = Rig::new()?;
    let (second_scope, mut other, other_boundary) = recovery_child(&second, 2, 1, key()?)?;
    assert_eq!(original.control.prepared_key(), other.control.prepared_key());
    assert_eq!(proof.scope_id(), other.scope_id());
    assert_eq!(proof.spec(), other.spec());
    assert_ne!(proof.execution_id(), second_scope.owner_id());
    assert!(!proof.matches(&other.control));
    assert!(proof.matches(&original.control));
    assert_eq!(proof.spec(), completion.spec);
    assert_eq!(proof.execution_id(), first_scope.owner_id());
    other.control.close();
    let other_completion = second.completion(&mut other.completion)?;
    assert!(matches!(other_completion.outcome, JobOutcome::Cancelled));
    drop((other_completion, other, other_boundary, completion, proof, original, original_boundary));
    drop((first, second));
    Ok(())
}
