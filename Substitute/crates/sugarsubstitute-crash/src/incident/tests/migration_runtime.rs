// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate real protected migration sources and report mutation fences without native effects.
use super::{
    migration_runtime_fixture::*,
    publication_fixture::{admitted, setup},
    *,
};
use sugarsubstitute_supervision::diagnostic_storage::*;

#[test]
fn actual_summary_migration_preserves_copy_and_fences_report_until_archive_settlement() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let base = original()?;
        let facts = base.facts();
        let summary = IncidentSummary::new(
            base.seed().clone(),
            base.revision(),
            SummaryFacts::new(
                RetentionFacts::new(
                    facts.retention().occurred(),
                    Some(IncidentUtc::from_unix_seconds(105)?),
                )?,
                facts.exit(),
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                facts.reporting(),
                facts.symbols().clone(),
            ),
        )?;
        let bytes = legacy(&summary)?;
        fixture.put(DiagnosticObject::Summary(summary.seed().key()), &bytes)?;
        let candidate = migrate_summary_v1(&bytes)?;
        let (mut prepared, journal) = prepare(&mut repository, &summary, &candidate, &context)?;
        let mut permit = activate_journal(&mut repository, &mut prepared, &journal, &context)?;
        let actual = repository.storage.complete_summary_migration(
            &mut permit,
            &IncidentSummaryMigrationValidator,
            &repository.operation,
            &context,
        )?;
        assert_eq!(actual.facts().file_identity(), journal.candidate_stage().identity());
        let viewed =
            ReportBinding::new(summary.seed().key(), summary.revision(), ReportSessionId::new(1)?);
        let effects = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.record_attempt(viewed, ReportAttempt::Presented, &context),
            Err(IncidentStoreError::Storage(_))
        ));
        assert!(matches!(
            repository.acknowledge(
                ReportCommand::new(viewed, ReportAction::Dismiss),
                IncidentUtc::from_unix_seconds(110)?,
                &context
            ),
            Err(IncidentStoreError::Storage(_))
        ));
        assert_eq!(fixture.snapshot().mutation_count(), effects);
        let archive = finish(&mut repository, &mut permit, &journal, &context)?;
        assert_eq!(archive.historical_operation(), journal.operation());
        let mut loaded = repository.storage.read(
            DiagnosticObject::Summary(summary.seed().key()),
            65536,
            &repository.operation,
            &context,
        )?;
        let loaded_source = loaded.whole_source(&context)?;
        assert_eq!(decode_summary(loaded_source.as_bytes())?.summary(), &summary);
        assert!(
            std::str::from_utf8(loaded_source.as_bytes())?.starts_with("{\"schema_version\":2,")
        );
        drop(loaded_source);
        loaded.retire(&context)?;
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.load(summary.seed().key(), &context),
            Err(IncidentStoreError::UnsupportedSchema(2))
        ));
        assert!(matches!(
            repository.record_attempt(viewed, ReportAttempt::Presented, &context),
            Err(IncidentStoreError::UnsupportedSchema(2))
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.storage.inventory(&repository.operation, &context)?;
        let mut read = repository.storage.read(
            DiagnosticObject::Journal(journal.operation()),
            65_536,
            &repository.operation,
            &context,
        )?;
        let recognized = repository.storage.recognize_summary_migration_archive(
            &mut read,
            &IncidentSummaryMigrationValidator,
            &repository.operation,
            &context,
        )?;
        assert_eq!(
            recognized.original_stage().sha256(),
            super::super::codec_ledger::digest(&bytes)
        );
        assert_eq!(
            recognized.original_stage().file_identity(),
            journal.original_stage().identity()
        );
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.load(summary.seed().key(), &context),
            Err(IncidentStoreError::UnsupportedSchema(2))
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}

#[test]
fn actual_candidate_matching_facts_cannot_substitute_for_exact_schema_conversion() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        fixture.put(DiagnosticObject::Summary(summary.seed().key()), &legacy(&summary)?)?;
        let facts = summary.facts();
        let changed = IncidentSummary::new(
            summary.seed().clone(),
            summary.revision(),
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                ReportDisposition::new(1, Some(ReportAttempt::Presented))?,
                facts.symbols().clone(),
            ),
        )?;
        let candidate = encode_summary(&StoredIncidentSummary::new(changed, None)?)?;
        let (mut prepared, journal) = prepare(&mut repository, &summary, &candidate, &context)?;
        let before = fixture.snapshot().mutation_count();
        assert!(
            repository
                .storage
                .seal_summary_migration(
                    &mut prepared,
                    plan(&journal)?,
                    &IncidentSummaryMigrationValidator,
                    &repository.operation,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(repository.snapshot().stages(), 2);
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Journal(journal.operation()))
                .is_none()
        );
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}

#[test]
fn actual_legacy_unknown_field_retains_decoder_cause_and_all_partial_owners() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        let valid = legacy(&summary)?;
        let mut value: serde_json::Value = serde_json::from_slice(&valid)?;
        value["content"]["external_json_sentinel_secret"] = true.into();
        let bytes = serde_json::to_vec(&value)?;
        fixture.put(DiagnosticObject::Summary(summary.seed().key()), &bytes)?;
        let (mut prepared, journal) =
            prepare(&mut repository, &summary, &migrate_summary_v1(&valid)?, &context)?;
        let failure = repository
            .storage
            .seal_summary_migration(
                &mut prepared,
                plan(&journal)?,
                &IncidentSummaryMigrationValidator,
                &repository.operation,
                &context,
            )
            .err()
            .ok_or("unexpected sealed journal")?;
        assert!(!format!("{failure}").contains("external_json_sentinel_secret"));
        assert!(!format!("{failure:?}").contains("external_json_sentinel_secret"));
        let mut current: Option<&dyn Error> = Some(&failure);
        let mut retained = false;
        while let Some(cause) = current {
            assert!(!cause.to_string().contains("external_json_sentinel_secret"));
            if let Some(json) = cause.downcast_ref::<serde_json::Error>() {
                retained |= json.to_string().contains("Unknown incident field")
                    && json.line() > 0
                    && json.column() > 0;
            }
            current = cause.source();
        }
        assert!(retained);
        assert_eq!(repository.snapshot().stages(), 2);
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(context.memory().claim(1).is_err());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}

#[test]
fn actual_two_to_three_migration_archives_untouched_bytes_before_current_pending_scan() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        let bytes = migrate_summary_v1(&legacy(&summary)?)?;
        let untouched = bytes.clone();
        fixture.put(DiagnosticObject::Summary(summary.seed().key()), &bytes)?;
        let mut work = None;
        assert!(repository.migrate_one_pending_summary(&mut work, &context)?);
        let actual = work.as_ref().ok_or("Missing actual migration owner")?;
        assert!(actual.completed());
        assert_eq!(actual.operation(), repository.operation());
        let active = fixture
            .snapshot()
            .active_identity(DiagnosticObject::Summary(summary.seed().key()))
            .ok_or("Current candidate absent")?;
        assert_eq!(
            active.sha256(),
            super::super::codec_ledger::digest(&migrate_summary_v2(&bytes)?)
        );
        assert_eq!(bytes, untouched);
        let mut archive = repository.storage.read(
            DiagnosticObject::Journal(repository.operation()),
            65536,
            &repository.operation,
            &context,
        )?;
        let mutations = fixture.snapshot().mutation_count();
        let outputs = fixture.snapshot().live_outputs();
        let stale = repository
            .storage
            .recognize_summary_migration_archive(
                &mut archive,
                &IncidentSummaryMigrationValidator,
                &repository.operation,
                &context,
            )
            .err()
            .ok_or("Stale inventory admitted archive recognition")?;
        assert_eq!(stale.code(), DiagnosticStorageFailureCode::InventoryIncomplete);
        assert_eq!(fixture.snapshot().mutation_count(), mutations);
        assert_eq!(fixture.snapshot().live_outputs(), outputs);
        repository.storage.inventory(&repository.operation, &context)?;
        let unclassified = repository
            .storage
            .validate_preserved_migration_inventory(&repository.operation, &context)
            .err()
            .ok_or("Unrecognized current-scan archive was classified")?;
        assert_eq!(unclassified.code(), DiagnosticStorageFailureCode::InventoryIncomplete);
        let recognized = repository.storage.recognize_summary_migration_archive(
            &mut archive,
            &IncidentSummaryMigrationValidator,
            &repository.operation,
            &context,
        )?;
        assert_eq!(
            recognized.original_stage().sha256(),
            super::super::codec_ledger::digest(&bytes)
        );
        repository
            .storage
            .validate_preserved_migration_inventory(&repository.operation, &context)?;
        let effects = fixture.snapshot().mutation_count();
        assert!(repository.migrate_one_pending_summary(&mut work, &context).is_err());
        assert_eq!(fixture.snapshot().mutation_count(), effects);
        let pending =
            repository.pending_incidents(IncidentUtc::from_unix_seconds(110)?, &context)?;
        drop(pending);
        assert_eq!(fixture.snapshot().mutation_count(), effects);
        fixture.put(
            DiagnosticObject::Journal(repository.operation()),
            b"{\"schema_version\":99,\"content\":{}}",
        )?;
        let effects = fixture.snapshot().mutation_count();
        assert!(
            repository.pending_incidents(IncidentUtc::from_unix_seconds(110)?, &context).is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), effects);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn two_to_three_partial_close_failure_retains_real_work_and_rejects_reissued_effects() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        let bytes = migrate_summary_v1(&legacy(&summary)?)?;
        fixture.put(DiagnosticObject::Summary(summary.seed().key()), &bytes)?;
        fixture.set_failure(fixture::DiagnosticFixtureFailure::Close, true);
        let mut work = None;
        assert!(repository.migrate_one_pending_summary(&mut work, &context).is_err());
        assert!(work.as_ref().is_some_and(|work| !work.completed()));
        assert!(fixture.snapshot().live_outputs() > 0);
        let effects = fixture.snapshot().mutation_count();
        assert!(repository.migrate_one_pending_summary(&mut work, &context).is_err());
        assert_eq!(fixture.snapshot().mutation_count(), effects);
        assert!(repository.retire(&context).is_err());
        fixture.set_failure(fixture::DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
