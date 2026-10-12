// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate real protected migration sources and report mutation fences without native effects.
use super::{
    migration_runtime_fixture::*,
    publication::{admitted, setup},
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
        let loaded = repository.load(summary.seed().key(), &context)?;
        assert_eq!(loaded.summary(), &summary);
        let advanced = repository.record_attempt(viewed, ReportAttempt::Presented, &context)?;
        assert_eq!(advanced.revision().get(), 3);
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
        assert_eq!(repository.load(summary.seed().key(), &context)?.summary(), advanced.summary());
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
            retained |= cause.to_string().contains("external_json_sentinel_secret");
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
