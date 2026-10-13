// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build legacy disk scenarios and prepare stages only through the actual storage port.
use super::*;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::diagnostic_storage::fixture::DiagnosticStorageFixture;
use sugarsubstitute_supervision::diagnostic_storage::*;
pub(super) type TestError = Box<dyn Error + Send + Sync>;
pub(super) type TestResult = Result<(), TestError>;

pub(super) fn original() -> Result<IncidentSummary, TestError> {
    super::summary().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}
pub(super) fn legacy(summary: &IncidentSummary) -> Result<Vec<u8>, TestError> {
    Ok(super::super::codec_encode::encode(
        1,
        super::super::codec_summary::SummaryRecord::from(summary),
        65_536,
    )?)
}
pub(super) fn facts(actual: DiagnosticReadFacts) -> Result<IncidentFileFacts, IncidentStoreError> {
    IncidentFileFacts::new(actual.file_identity(), actual.sha256(), actual.bytes())
}
fn recorded(actual: IncidentFileFacts) -> Result<RecordedDiagnosticFileFacts, TestError> {
    Ok(RecordedDiagnosticFileFacts::new(actual.identity(), actual.sha256(), actual.bytes())?)
}
pub(super) fn plan(
    journal: &SummaryMigrationJournal,
) -> Result<DiagnosticSummaryMigrationPlan, TestError> {
    let bytes =
        encode_store_state(&IncidentStoreState::SummaryMigration(Box::new(journal.clone())))?;
    Ok(DiagnosticSummaryMigrationPlan::new(
        journal.operation(),
        journal.incident(),
        journal.revision(),
        journal.payload_revision(),
        recorded(journal.original_file())?,
        recorded(journal.original_stage())?,
        recorded(journal.candidate_stage())?,
        super::super::codec_ledger::digest(&bytes),
    )?)
}
pub(super) fn prepare(
    repository: &mut IncidentRepository,
    summary: &IncidentSummary,
    candidate: &[u8],
    context: &JobContext,
) -> Result<(DiagnosticSummaryMigrationPreparation, SummaryMigrationJournal), TestError> {
    repository.storage.inventory(&repository.operation, context)?;
    let mut read = repository.storage.read(
        DiagnosticObject::Summary(summary.seed().key()),
        65_536,
        &repository.operation,
        context,
    )?;
    let mut prepared =
        repository.storage.prepare_summary_migration(&mut read, &repository.operation, context)?;
    prepared.write_candidate(candidate, context)?;
    let actual_candidate = prepared.flush_candidate(context)?.read_facts();
    let journal = SummaryMigrationJournal::new(
        repository.operation(),
        summary.seed().key(),
        summary.revision(),
        None,
        facts(prepared.original_file())?,
        facts(prepared.original_stage().read_facts())?,
        facts(actual_candidate)?,
    )?;
    Ok((prepared, journal))
}
pub(super) fn activate_journal(
    repository: &mut IncidentRepository,
    prepared: &mut DiagnosticSummaryMigrationPreparation,
    journal: &SummaryMigrationJournal,
    context: &JobContext,
) -> Result<DiagnosticSummaryMigrationPermit, TestError> {
    let mut stage = repository.storage.seal_summary_migration(
        prepared,
        plan(journal)?,
        &IncidentSummaryMigrationValidator,
        &repository.operation,
        context,
    )?;
    let bytes =
        encode_store_state(&IncidentStoreState::SummaryMigration(Box::new(journal.clone())))?;
    stage.write_chunk(&bytes, context)?;
    stage.flush(context)?;
    Ok(repository.storage.activate_summary_migration(&mut stage, &repository.operation, context)?)
}
pub(super) fn finish(
    repository: &mut IncidentRepository,
    permit: &mut DiagnosticSummaryMigrationPermit,
    journal: &SummaryMigrationJournal,
    context: &JobContext,
) -> Result<DiagnosticSummaryMigrationArchiveCommit, TestError> {
    let actual = repository.storage.complete_summary_migration(
        permit,
        &IncidentSummaryMigrationValidator,
        &repository.operation,
        context,
    )?;
    let archive = SummaryMigrationArchive::new(journal.clone())?;
    let bytes =
        encode_store_state(&IncidentStoreState::SummaryMigrationArchive(Box::new(archive)))?;
    Ok(repository.storage.finish_summary_migration(
        permit,
        &actual,
        &bytes,
        &IncidentSummaryMigrationValidator,
        &repository.operation,
        context,
    )?)
}

pub(super) fn restart_journal(
    fixture: &DiagnosticStorageFixture,
    repository: &IncidentRepository,
    candidate_current: bool,
) -> Result<SummaryMigrationJournal, TestError> {
    let summary = original()?;
    let original_bytes = legacy(&summary)?;
    let candidate_bytes = migrate_summary_v1(&original_bytes)?;
    let object = DiagnosticObject::Summary(summary.seed().key());
    let current = repository.operation();
    let historical = LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: current.owner().namespace,
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        IncidentOperationId::new(9)?,
    );
    let original = fixture.put(object, &original_bytes)?;
    let copy = fixture.put_stage(object, historical, &original_bytes)?;
    let candidate = if candidate_current {
        fixture.put(object, &candidate_bytes)?
    } else {
        fixture.put_stage(object, historical, &candidate_bytes)?
    };
    let journal = SummaryMigrationJournal::new(
        historical,
        summary.seed().key(),
        summary.revision(),
        None,
        facts(original)?,
        facts(copy)?,
        facts(candidate)?,
    )?;
    fixture.put(
        DiagnosticObject::Journal(historical),
        &encode_store_state(&IncidentStoreState::SummaryMigration(Box::new(journal.clone())))?,
    )?;
    Ok(journal)
}

pub(super) fn recover(
    repository: &mut IncidentRepository,
    journal: &SummaryMigrationJournal,
    context: &JobContext,
) -> Result<DiagnosticSummaryMigrationPermit, TestError> {
    repository.storage.inventory(&repository.operation, context)?;
    let mut journal_read = repository.storage.read(
        DiagnosticObject::Journal(journal.operation()),
        65_536,
        &repository.operation,
        context,
    )?;
    let mut summary_read = repository.storage.read(
        DiagnosticObject::Summary(journal.incident()),
        65_536,
        &repository.operation,
        context,
    )?;
    Ok(repository.storage.recover_summary_migration(
        &mut journal_read,
        &mut summary_read,
        &IncidentSummaryMigrationValidator,
        &repository.operation,
        context,
    )?)
}
