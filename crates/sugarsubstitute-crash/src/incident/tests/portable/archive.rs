// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copied native IDs remain inert; existing active-store recovery still rejects them.
use super::*;
use crate::incident::codec_ledger::digest;

pub(super) fn archived(open: bool) -> Result<(Source, SummaryMigrationJournal), TestError> {
    let mut source = Source::new()?;
    let stored = base_summary()?;
    let summary = stored.summary();
    let original = super::super::migration_runtime_fixture::legacy(summary)?;
    let candidate = migrate_summary_v1(&original)?;
    let operation = LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([9; 16])?,
        },
        IncidentOperationId::new(10)?,
    );
    let journal = SummaryMigrationJournal::for_step(
        SummaryMigrationStep::OneToTwo,
        operation,
        summary.seed().key(),
        summary.revision(),
        None,
        IncidentFileFacts::new([11; 24], digest(&original), original.len() as u64)?,
        IncidentFileFacts::new([12; 24], digest(&original), original.len() as u64)?,
        IncidentFileFacts::new([13; 24], digest(&candidate), candidate.len() as u64)?,
    )?;
    source.summary(stored)?;
    source.add(DiagnosticEntryClass::Stage, None, original)?;
    let state = if open {
        IncidentStoreState::SummaryMigration(Box::new(journal.clone()))
    } else {
        IncidentStoreState::SummaryMigrationArchive(Box::new(SummaryMigrationArchive::new(
            journal.clone(),
        )?))
    };
    source.add(
        DiagnosticEntryClass::Journal,
        Some(DiagnosticObject::Journal(operation)),
        encode_store_state(&state)?,
    )?;
    Ok((source.copied(), journal))
}

#[test]
fn completed_archive_copy_is_passive_readable_while_native_replay_keeps_identity_checks()
-> TestResult {
    admitted(|context| {
        let (mut source, journal) = archived(false)?;
        let bytes = source.original_bytes();
        let (fixture, mut repository) = super::super::publication_fixture::setup(&context)?;
        for row in &source.rows {
            if row.class == DiagnosticEntryClass::Stage {
                fixture.put_stage(
                    DiagnosticObject::Summary(journal.incident()),
                    journal.operation(),
                    &row.bytes,
                )?;
            } else if let Some(object) = row.object {
                fixture.put(object, &row.bytes)?;
            }
        }
        let actual = fixture
            .snapshot()
            .active_identity(DiagnosticObject::Summary(journal.incident()))
            .ok_or("Copied Summary absent")?;
        assert_ne!(actual.file_identity(), journal.original_file().identity());
        let cause = match repository.pending_incidents(now()?, &context) {
            Ok(_) => return Err("Copied archive gained native recovery authority".into()),
            Err(cause) => cause,
        };
        let IncidentStoreError::Storage(cause) = cause else {
            return Err("Archive failed before its actual native identity check".into());
        };
        assert_eq!(
            cause.code(),
            sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageFailureCode::Read
        );
        assert!(std::error::Error::source(&cause).is_some());
        drop(cause);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        let history =
            project_portable_committed(&mut source, NamespaceId::new([1; 32]), now()?, &context)?;
        assert_eq!(history.summaries().len(), 1);
        assert_eq!(
            history.summaries().next().ok_or("History missing")?.summary().seed().key(),
            journal.incident()
        );
        drop(history);
        assert!(source.unchanged(&bytes));
        Ok(())
    })
}

#[test]
fn unfinished_journal_and_missing_preserved_original_refuse_the_complete_projection() -> TestResult
{
    admitted(|context| {
        let (mut source, _) = archived(true)?;
        let bytes = source.original_bytes();
        let failure = match project_portable_committed(
            &mut source,
            NamespaceId::new([1; 32]),
            now()?,
            &context,
        ) {
            Ok(_) => return Err("Unfinished journal became committed history".into()),
            Err(cause) => cause,
        };
        assert_eq!(failure.code(), PortableIncidentFailureCode::Uncommitted);
        drop(failure);
        assert!(source.unchanged(&bytes));
        let (mut source, _) = archived(false)?;
        let stage = source
            .rows
            .iter_mut()
            .find(|row| row.class == DiagnosticEntryClass::Stage)
            .ok_or("Preserved original absent")?;
        stage.bytes[0] ^= 1;
        let failure = match project_portable_committed(
            &mut source,
            NamespaceId::new([1; 32]),
            now()?,
            &context,
        ) {
            Ok(_) => return Err("Changed preserved original accepted".into()),
            Err(cause) => cause,
        };
        assert_eq!(failure.code(), PortableIncidentFailureCode::Uncommitted);
        Ok(())
    })
}
