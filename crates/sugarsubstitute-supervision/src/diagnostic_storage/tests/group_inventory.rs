// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Count immutable physical generations through the actual storage and execution owners.
use super::*;
use crate::diagnostic_storage::fixture::DiagnosticFixtureFailure;

fn payload(revision: u64, kind: AttachmentKind) -> Result<DiagnosticObject, TestError> {
    Ok(DiagnosticObject::Payload(incident()?, IncidentRevision::new(revision)?, kind))
}
fn generations(filesystem: &Filesystem, count: u64) -> TestResult {
    for revision in 1..=count {
        filesystem.put_checked(payload(revision, AttachmentKind::Log)?, None, b"x")?;
    }
    Ok(())
}

#[test]
fn diagnostic_generation_inventory_preserves_and_blocks_thirty_three_same_subject_groups()
-> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        generations(&filesystem, 33)?;
        let original: Vec<_> = (1..=33)
            .map(|revision| {
                let object = payload(revision, AttachmentKind::Log)?;
                Ok((
                    object,
                    filesystem.snapshot().active_identity(object).ok_or("missing original")?,
                ))
            })
            .collect::<Result<_, TestError>>()?;
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Incomplete);
        assert_eq!(inventory.payload_groups(), 33);
        assert_eq!(inventory.payload_bytes(), 33);
        assert_eq!(
            storage
                .stage(payload(34, AttachmentKind::Log)?, 1, &operation, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::InventoryIncomplete)
        );
        assert_eq!(filesystem.snapshot().mutation_count(), 0);
        for (object, facts) in original {
            assert_eq!(filesystem.snapshot().active_identity(object), Some(facts));
        }
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_generation_full_group_admission_rejects_new_revision_before_create() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        generations(&filesystem, 32)?;
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        assert_eq!(storage.inventory(&operation, &context)?.payload_groups(), 32);
        let before = filesystem.snapshot().mutation_count();
        assert_eq!(
            storage
                .stage(payload(33, AttachmentKind::Log)?, 1, &operation, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::Capacity)
        );
        assert_eq!(filesystem.snapshot().mutation_count(), before);
        let mut same =
            storage.stage(payload(1, AttachmentKind::Fault)?, 1, &operation, &context)?;
        assert_eq!(filesystem.snapshot().mutation_count(), before + 1);
        same.retire(&context)?;
        assert_eq!(storage.inventory(&operation, &context)?.payload_groups(), 32);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_generation_active_kinds_and_abandoned_stages_share_only_their_immutable_revision()
-> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        for kind in [AttachmentKind::Log, AttachmentKind::Fault] {
            filesystem.put_checked(payload(1, kind)?, None, b"x")?;
        }
        filesystem.put_checked(payload(1, AttachmentKind::Log)?, Some(key(1)?), b"x")?;
        filesystem.put_checked(payload(2, AttachmentKind::Log)?, Some(key(2)?), b"x")?;
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.status(), DiagnosticInventoryStatus::Complete);
        assert_eq!(inventory.payload_groups(), 2);
        assert_eq!(inventory.payload_bytes(), 4);
        assert_eq!(filesystem.snapshot().mutation_count(), 0);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_generation_partial_create_and_failed_cleanup_keep_actual_group_and_claim()
-> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        generations(&filesystem, 31)?;
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        filesystem.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
        assert_eq!(
            storage
                .stage(payload(32, AttachmentKind::Log)?, 1, &operation, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::Write)
        );
        assert_eq!(filesystem.snapshot().live_outputs(), 1);
        assert!(context.memory().claim(1).is_err());
        let before = filesystem.snapshot().mutation_count();
        assert_eq!(
            storage
                .stage(payload(33, AttachmentKind::Log)?, 1, &operation, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::Capacity)
        );
        assert_eq!(filesystem.snapshot().mutation_count(), before);
        assert_eq!(storage.inventory(&operation, &context)?.payload_groups(), 32);
        filesystem.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(storage.retire(&operation, &context).is_err());
        assert_eq!(filesystem.snapshot().live_outputs(), 1);
        assert!(context.memory().claim(1).is_err());
        filesystem.set_failure(DiagnosticFixtureFailure::Close, false);
        storage.retire(&operation, &context)?;
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        for revision in 1..=31 {
            assert!(
                filesystem
                    .snapshot()
                    .active_identity(payload(revision, AttachmentKind::Log)?)
                    .is_some()
            );
        }
        Ok(())
    })
}

#[test]
fn diagnostic_generation_checked_stage_close_preserves_its_physical_generation_charge() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        generations(&filesystem, 31)?;
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let mut stage =
            storage.stage(payload(32, AttachmentKind::Log)?, 1, &operation, &context)?;
        stage.retire(&context)?;
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        assert_eq!(storage.inventory(&operation, &context)?.payload_groups(), 32);
        let before = filesystem.snapshot().mutation_count();
        assert_eq!(
            storage
                .stage(payload(33, AttachmentKind::Log)?, 1, &operation, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::Capacity)
        );
        assert_eq!(filesystem.snapshot().mutation_count(), before);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_generation_different_subjects_do_not_share_the_same_revision_group() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        for epoch in 2..=3 {
            filesystem.put_checked(
                DiagnosticObject::Payload(
                    IncidentKey::new(IncidentSubject::Supervisor(owner_key(epoch)?)),
                    IncidentRevision::new(1)?,
                    AttachmentKind::Log,
                ),
                None,
                b"x",
            )?;
        }
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        assert_eq!(storage.inventory(&operation, &context)?.payload_groups(), 2);
        assert_eq!(filesystem.snapshot().mutation_count(), 0);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
