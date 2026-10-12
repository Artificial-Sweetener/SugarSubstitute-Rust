// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover historical migration data under actual new protected operation custody.
use super::{
    migration_runtime_fixture::*,
    publication::{admitted, setup},
    *,
};
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};

#[test]
fn original_migration_recovery_adopts_exact_existing_candidate_without_restaging() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let journal = restart_journal(&fixture, &repository, false)?;
        let effects = fixture.snapshot().mutation_count();
        let mut permit = recover(&mut repository, &journal, &context)?;
        assert_ne!(permit.current_operation(), permit.historical_operation());
        assert_eq!(permit.current_operation(), repository.operation());
        assert_eq!(repository.snapshot().stages(), 1);
        repository.storage.recover_summary_migration_candidate(
            &mut permit,
            &IncidentSummaryMigrationValidator,
            &repository.operation,
            &context,
        )?;
        assert_eq!(repository.snapshot().stages(), 2);
        assert_eq!(fixture.snapshot().mutation_count(), effects);
        let completed = finish(&mut repository, &mut permit, &journal, &context)?;
        assert_eq!(completed.historical_operation(), journal.operation());
        let current = fixture
            .snapshot()
            .active_identity(DiagnosticObject::Summary(journal.incident()))
            .ok_or("candidate absent")?;
        assert_eq!(current.file_identity(), journal.candidate_stage().identity());
        assert_eq!(current.sha256(), journal.candidate_stage().sha256());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}

#[test]
fn candidate_migration_recovery_reconciles_actual_inode_without_replaying_summary() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let journal = restart_journal(&fixture, &repository, true)?;
        let mut permit = recover(&mut repository, &journal, &context)?;
        let before = fixture.snapshot().mutation_count();
        assert!(
            repository
                .storage
                .recover_summary_migration_candidate(
                    &mut permit,
                    &IncidentSummaryMigrationValidator,
                    &repository.operation,
                    &context
                )
                .is_err()
        );
        let actual = repository.storage.complete_summary_migration(
            &mut permit,
            &IncidentSummaryMigrationValidator,
            &repository.operation,
            &context,
        )?;
        assert_eq!(actual.facts().file_identity(), journal.candidate_stage().identity());
        assert_eq!(fixture.snapshot().mutation_count(), before);
        finish(&mut repository, &mut permit, &journal, &context)?;
        assert_eq!(
            repository.load(journal.incident(), &context)?.summary().revision(),
            journal.revision()
        );
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}

#[test]
fn replacement_summary_inode_is_a_preserved_third_state_even_with_identical_bytes() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let journal = restart_journal(&fixture, &repository, false)?;
        fixture.put(DiagnosticObject::Summary(journal.incident()), &legacy(&original()?)?)?;
        let before = fixture.snapshot().mutation_count();
        assert!(recover(&mut repository, &journal, &context).is_err());
        assert_eq!(repository.snapshot().reads(), 2);
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(repository.snapshot().stages(), 0);
        assert!(context.memory().claim(1).is_err());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}

#[test]
fn candidate_recovery_close_failure_preserves_custody_and_cannot_return_activation() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let journal = restart_journal(&fixture, &repository, true)?;
        let mut permit = recover(&mut repository, &journal, &context)?;
        let before = fixture.snapshot().mutation_count();
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(
            repository
                .storage
                .complete_summary_migration(
                    &mut permit,
                    &IncidentSummaryMigrationValidator,
                    &repository.operation,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(repository.snapshot().reads(), 2);
        assert_eq!(repository.snapshot().stages(), 1);
        assert!(repository.snapshot().writer_held());
        assert!(repository.retire(&context).is_err());
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
