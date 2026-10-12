// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe protected source custody, exact CAS and preserved archive bytes through real owner phases.
use super::*;
#[test]
fn diagnostic_migration_normal_preserves_original_and_recognizes_archive_idempotently() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut permit, validator) = normal(&mut storage, &operation, &context)?;
        assert!(storage.snapshot().disposition_retained());
        assert!(
            storage
                .stage(DiagnosticObject::Summary(incident()?), 64, &operation, &context)
                .is_err()
        );
        let candidate =
            storage.complete_summary_migration(&mut permit, &validator, &operation, &context)?;
        let archive = storage.finish_summary_migration(
            &mut permit,
            &candidate,
            ARCHIVE,
            &validator,
            &operation,
            &context,
        )?;
        assert!(!storage.snapshot().disposition_retained());
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        let inventory = storage.inventory(&operation, &context)?;
        assert_eq!(inventory.payload_bytes(), LEGACY.len() as u64);
        for _ in 0..2 {
            let mut read = storage.read(
                DiagnosticObject::Journal(permit.historical_operation()),
                64,
                &operation,
                &context,
            )?;
            let observed = storage
                .recognize_summary_migration_archive(&mut read, &validator, &operation, &context)?;
            assert_eq!(recorded(observed.original_stage())?, permit.plan().original_stage());
            assert_eq!(filesystem.snapshot().live_outputs(), 0);
        }
        assert_eq!(lock(&storage.shared).archives.len(), 1);
        assert_eq!(
            filesystem
                .snapshot()
                .active_identity(DiagnosticObject::Journal(permit.historical_operation())),
            Some(archive.facts())
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_migration_post_summary_and_archive_rename_retry_never_replays_effects() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut permit, validator) = normal(&mut storage, &operation, &context)?;
        filesystem.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(
            storage
                .complete_summary_migration(&mut permit, &validator, &operation, &context)
                .is_err()
        );
        let mutations = filesystem.snapshot().mutation_count();
        assert_eq!(
            filesystem
                .snapshot()
                .active_identity(DiagnosticObject::Summary(incident()?))
                .map(|facts| recorded(facts).ok()),
            Some(Some(permit.plan().candidate_stage()))
        );
        filesystem.set_failure(DiagnosticFixtureFailure::PostActivation, false);
        let candidate =
            storage.complete_summary_migration(&mut permit, &validator, &operation, &context)?;
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        filesystem.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(
            storage
                .finish_summary_migration(
                    &mut permit,
                    &candidate,
                    ARCHIVE,
                    &validator,
                    &operation,
                    &context
                )
                .is_err()
        );
        let mutations = filesystem.snapshot().mutation_count();
        assert!(storage.snapshot().disposition_retained());
        filesystem.set_failure(DiagnosticFixtureFailure::PostActivation, false);
        storage.finish_summary_migration(
            &mut permit,
            &candidate,
            ARCHIVE,
            &validator,
            &operation,
            &context,
        )?;
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_migration_recovery_original_and_candidate_close_actual_read_before_settlement()
-> TestResult {
    for candidate in [false, true] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let validator = recovery(&filesystem, candidate)?;
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let mut journal = storage.read(
                DiagnosticObject::Journal(validator.plan.historical_operation()),
                64,
                &operation,
                &context,
            )?;
            let mut summary =
                storage.read(DiagnosticObject::Summary(incident()?), 64, &operation, &context)?;
            let mut permit = storage.recover_summary_migration(
                &mut journal,
                &mut summary,
                &validator,
                &operation,
                &context,
            )?;
            let mutations = filesystem.snapshot().mutation_count();
            if candidate {
                assert!(
                    storage
                        .recover_summary_migration_candidate(
                            &mut permit,
                            &validator,
                            &operation,
                            &context
                        )
                        .is_err()
                );
                filesystem.set_failure(DiagnosticFixtureFailure::Close, true);
                assert!(
                    storage
                        .complete_summary_migration(&mut permit, &validator, &operation, &context)
                        .is_err()
                );
                assert_eq!(storage.snapshot().reads(), 2);
                assert_eq!(filesystem.snapshot().mutation_count(), mutations);
                filesystem.set_failure(DiagnosticFixtureFailure::Close, false);
            } else {
                storage.recover_summary_migration_candidate(
                    &mut permit,
                    &validator,
                    &operation,
                    &context,
                )?;
            }
            let active = storage.complete_summary_migration(
                &mut permit,
                &validator,
                &operation,
                &context,
            )?;
            assert_eq!(recorded(active.facts())?, validator.plan.candidate_stage());
            assert_eq!(filesystem.snapshot().mutation_count(), mutations + usize::from(!candidate));
            storage.finish_summary_migration(
                &mut permit,
                &active,
                ARCHIVE,
                &validator,
                &operation,
                &context,
            )?;
            storage.retire(&operation, &context)?;
            assert_eq!(filesystem.snapshot().live_outputs(), 0);
            Ok(())
        })?;
    }
    Ok(())
}
