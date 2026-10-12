// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve failed custody and charge only actual archived original-copy bytes once.
use super::*;
#[test]
fn diagnostic_migration_archive_recognition_after_owner_retirement_ignores_newer_summary()
-> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut permit, validator) = normal(&mut storage, &operation, &context)?;
        let active =
            storage.complete_summary_migration(&mut permit, &validator, &operation, &context)?;
        storage.finish_summary_migration(
            &mut permit,
            &active,
            ARCHIVE,
            &validator,
            &operation,
            &context,
        )?;
        storage.retire(&operation, &context)?;
        drop((storage, operation, permit, active));
        filesystem.put_checked(
            DiagnosticObject::Summary(incident()?),
            None,
            b"newer immutable revision",
        )?;
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let mutations = filesystem.snapshot().mutation_count();
        let mut archive = storage.read(
            DiagnosticObject::Journal(validator.plan.historical_operation()),
            64,
            &operation,
            &context,
        )?;
        storage.recognize_summary_migration_archive(
            &mut archive,
            &validator,
            &operation,
            &context,
        )?;
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert_eq!(storage.inventory(&operation, &context)?.payload_bytes(), LEGACY.len() as u64);
        let mut archive = storage.read(
            DiagnosticObject::Journal(validator.plan.historical_operation()),
            64,
            &operation,
            &context,
        )?;
        storage.recognize_summary_migration_archive(
            &mut archive,
            &validator,
            &operation,
            &context,
        )?;
        assert_eq!(lock(&storage.shared).archives.len(), 1);
        storage.retire(&operation, &context)?;
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn diagnostic_migration_partial_create_and_close_failure_keep_actual_claim_and_intent() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let mut original =
            storage.read(DiagnosticObject::Summary(incident()?), 64, &operation, &context)?;
        filesystem.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
        assert!(storage.prepare_summary_migration(&mut original, &operation, &context).is_err());
        assert!(storage.snapshot().disposition_retained());
        assert_eq!(storage.snapshot().stages(), 2);
        assert_eq!(storage.snapshot().live_handles(), 2);
        assert_eq!(
            lock(&storage.shared).stages.iter().map(|stage| stage.reserved).sum::<u64>(),
            2 * DIAGNOSTIC_IO_BYTES as u64
        );
        filesystem.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(storage.retire(&operation, &context).is_err());
        assert_eq!(filesystem.snapshot().live_outputs(), 2);
        filesystem.set_failure(DiagnosticFixtureFailure::Close, false);
        storage.retire(&operation, &context)?;
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        assert!(!storage.snapshot().disposition_retained());
        Ok(())
    })
}
#[test]
fn diagnostic_migration_missing_or_tampered_original_copy_blocks_without_candidate_effect()
-> TestResult {
    for change in 0..4 {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let validator = recovery(&filesystem, false)?;
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let mut journal = storage.read(
                DiagnosticObject::Journal(validator.plan.historical_operation()),
                64,
                &operation,
                &context,
            )?;
            let mut summary =
                storage.read(DiagnosticObject::Summary(incident()?), 64, &operation, &context)?;
            filesystem.change_stage(change);
            let mutations = filesystem.snapshot().mutation_count();
            assert!(
                storage
                    .recover_summary_migration(
                        &mut journal,
                        &mut summary,
                        &validator,
                        &operation,
                        &context
                    )
                    .is_err()
            );
            assert_eq!(filesystem.snapshot().mutation_count(), mutations);
            assert!(storage.snapshot().disposition_retained());
            assert!(storage.snapshot().live_handles() >= 2);
            storage.retire(&operation, &context)?;
            assert_eq!(filesystem.snapshot().live_outputs(), 0);
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_migration_128_recognitions_reject_129_preparation_before_create() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let object = DiagnosticObject::Summary(incident()?);
        filesystem.put(object, LEGACY);
        let mut validators = Vec::new();
        let mut old_identity = [0; 24];
        old_identity[0] = 1;
        let mut next_identity = [0; 24];
        next_identity[0] = 2;
        for id in 1..=128 {
            let historical = key(id)?;
            let copy = filesystem.put_stage(object, historical, LEGACY);
            let plan = DiagnosticSummaryMigrationPlan::new(
                historical,
                incident()?,
                IncidentRevision::new(1)?,
                None,
                RecordedDiagnosticFileFacts::new(
                    old_identity,
                    Sha256::digest(LEGACY).into(),
                    LEGACY.len() as u64,
                )?,
                recorded(copy)?,
                RecordedDiagnosticFileFacts::new(
                    next_identity,
                    Sha256::digest(CANDIDATE).into(),
                    CANDIDATE.len() as u64,
                )?,
                Sha256::digest(JOURNAL).into(),
            )?;
            filesystem.put(DiagnosticObject::Journal(historical), ARCHIVE);
            validators.push(Validator::new(plan));
        }
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        for validator in &validators {
            let mut read = storage.read(
                DiagnosticObject::Journal(validator.plan.historical_operation()),
                64,
                &operation,
                &context,
            )?;
            storage
                .recognize_summary_migration_archive(&mut read, validator, &operation, &context)?;
        }
        assert_eq!(lock(&storage.shared).archives.len(), 128);
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        let mut original = storage.read(object, 64, &operation, &context)?;
        let mutations = filesystem.snapshot().mutation_count();
        assert_eq!(
            storage
                .prepare_summary_migration(&mut original, &operation, &context)
                .err()
                .ok_or("129 admitted")?
                .code(),
            Code::Capacity
        );
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert_eq!(storage.snapshot().stages(), 0);
        original.retire(&context)?;
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
