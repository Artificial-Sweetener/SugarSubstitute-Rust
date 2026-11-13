// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual bounded sender transitions using data-only native observations.
use super::*;
fn operation(value: u64) -> Result<LifecyclePipeOperationId, LifecyclePipeError> {
    LifecyclePipeOperationId::new(value)
}
fn written(plan: &mut SendPlan, id: u64, role: Role) -> Result<(), Box<dyn std::error::Error>> {
    let operation = operation(id)?;
    plan.install(operation, 192, role)?;
    plan.issued(operation)?;
    assert_eq!(plan.complete(operation, 192)?, role);
    Ok(())
}
#[test]
fn root_recovery_sender_advances_identity_fragments_root_and_commit_only_after_complete_written()
-> Result<(), Box<dyn std::error::Error>> {
    let mut plan = SendPlan::default();
    let first = operation(1)?;
    plan.install(first, 192, Role::Identity)?;
    assert_eq!(plan.stage, Stage::Identity);
    assert_eq!(plan.sequence, 0);
    assert!(plan.complete(first, 192).is_err());
    assert_eq!(plan.stage, Stage::Identity);
    plan.issued(first)?;
    assert_eq!(plan.stage, Stage::Identity);
    assert_eq!(plan.sequence, 0);
    assert!(plan.complete(first, 191).is_err());
    assert!(plan.complete(operation(2)?, 192).is_err());
    assert_eq!(plan.stage, Stage::Identity);
    assert_eq!(plan.sequence, 0);
    plan.complete(first, 192)?;
    assert_eq!(plan.stage, Stage::Startup(0));
    assert_eq!(plan.sequence, 1);
    written(
        &mut plan,
        2,
        Role::Startup { next: RECOVERY_STARTUP_CHUNK, total: RECOVERY_STARTUP_BYTES },
    )?;
    assert_eq!(plan.stage, Stage::Startup(RECOVERY_STARTUP_CHUNK));
    written(
        &mut plan,
        3,
        Role::Startup { next: RECOVERY_STARTUP_BYTES, total: RECOVERY_STARTUP_BYTES },
    )?;
    assert_eq!(plan.stage, Stage::Root(0));
    written(
        &mut plan,
        4,
        Role::Root { next: RECOVERY_ROOT_CHUNK / 2, total: RECOVERY_ROOT_BYTES / 2 },
    )?;
    assert_eq!(plan.stage, Stage::Root(RECOVERY_ROOT_CHUNK / 2));
    written(
        &mut plan,
        5,
        Role::Root { next: RECOVERY_ROOT_BYTES / 2, total: RECOVERY_ROOT_BYTES / 2 },
    )?;
    assert_eq!(plan.stage, Stage::Commit);
    written(&mut plan, 6, Role::Commit)?;
    assert_eq!(plan.stage, Stage::Active);
    assert_eq!(plan.sequence, 6);
    written(&mut plan, 7, Role::Enrolled)?;
    written(&mut plan, 8, Role::Stop)?;
    assert_eq!(plan.stage, Stage::Active);
    assert_eq!(plan.sequence, 8);
    Ok(())
}
#[test]
fn root_recovery_sender_revoke_does_not_advance_and_issued_write_cannot_be_relabelled_revoked()
-> Result<(), Box<dyn std::error::Error>> {
    let mut plan = SendPlan::default();
    let id = operation(1)?;
    plan.install(id, 192, Role::Identity)?;
    plan.revoked(id)?;
    assert!(!plan.pending());
    assert_eq!(plan.stage, Stage::Identity);
    assert_eq!(plan.sequence, 0);
    let id = operation(2)?;
    plan.install(id, 192, Role::Identity)?;
    plan.issued(id)?;
    assert!(plan.issued(id).is_err());
    assert!(plan.revoked(id).is_err());
    assert!(plan.pending());
    assert_eq!(plan.stage, Stage::Identity);
    plan.complete(id, 192)?;
    assert_eq!(plan.sequence, 1);
    assert_eq!(plan.stage, Stage::Startup(0));
    Ok(())
}
#[test]
fn root_recovery_sender_refuses_sequence_exhaustion_and_invalid_offset_without_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    let mut plan = SendPlan { sequence: u64::MAX, ..Default::default() };
    assert!(plan.install(operation(1)?, 192, Role::Identity).is_err());
    assert!(!plan.pending());
    plan.sequence = 0;
    written(&mut plan, 1, Role::Identity)?;
    let id = operation(2)?;
    plan.install(id, 192, Role::Startup { next: 0, total: 10 })?;
    plan.issued(id)?;
    assert!(plan.complete(id, 192).is_err());
    assert_eq!(plan.stage, Stage::Startup(0));
    assert_eq!(plan.sequence, 1);
    assert!(plan.pending());
    assert!(plan.install(operation(3)?, 192, Role::Commit).is_err());
    Ok(())
}
