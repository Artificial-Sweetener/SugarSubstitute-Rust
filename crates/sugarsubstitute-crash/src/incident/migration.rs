// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact historical migration bytes supplied by the sole protected storage owner.
use super::{
    IncidentFileFacts, IncidentStoreError, IncidentStoreState, StoredIncidentSummary,
    SummaryMigrationJournal, codec_ledger::digest, decode_store_state, decode_summary,
    encode_store_state, migrate_summary_v1,
};
use sugarsubstitute_contracts::incident::LogicalStoreOperationKey;
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticEntryClass, DiagnosticObject, DiagnosticReadFacts, DiagnosticRecoverySource,
    DiagnosticRecoveryValidationCode as Code, DiagnosticRecoveryValidationError as ValidationError,
    DiagnosticSummaryMigrationArchiveDecision, DiagnosticSummaryMigrationCase,
    DiagnosticSummaryMigrationDecision, DiagnosticSummaryMigrationPlan,
    DiagnosticSummaryMigrationValidator, RecordedDiagnosticFileFacts,
};

/// Closed summary migration semantic adapter; only storage supplies actual read/stage observations.
/// Returned plans and decisions are inert; native custody, names, fencing and CAS remain with storage.
pub struct IncidentSummaryMigrationValidator;
impl DiagnosticSummaryMigrationValidator for IncidentSummaryMigrationValidator {
    fn validate_preparation(
        &self,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original: &DiagnosticRecoverySource,
        original_stage: &DiagnosticRecoverySource,
        candidate_stage: &DiagnosticRecoverySource,
    ) -> Result<(), ValidationError> {
        let journal = planned(current, plan)?;
        summary_source(original, DiagnosticEntryClass::Summary, &journal)?;
        actual(original, journal.original_file())?;
        original_copy(original_stage, &journal)?;
        if original.as_bytes() != original_stage.as_bytes() {
            return Err(ValidationError::rejected(Code::InvalidIdentity));
        }
        conversion(candidate_stage, DiagnosticEntryClass::Stage, original_stage, &journal)
    }
    fn validate_recovery(
        &self,
        current: LogicalStoreOperationKey,
        source: &DiagnosticRecoverySource,
        summary: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationDecision, ValidationError> {
        journal_source(source)?;
        let state = decode_store_state(source.as_bytes()).map_err(codec_error)?;
        let IncidentStoreState::SummaryMigration(journal) = &state else {
            return Err(ValidationError::rejected(Code::InvalidJournal));
        };
        canonical(source, &state)?;
        historical(current, source, journal)?;
        summary_source(summary, DiagnosticEntryClass::Summary, journal)?;
        let case = if matches(summary.facts(), journal.original_file()) {
            original_semantics(summary.as_bytes(), journal)?;
            DiagnosticSummaryMigrationCase::Original
        } else if matches(summary.facts(), journal.candidate_stage()) {
            let stored = decode_summary(summary.as_bytes()).map_err(codec_error)?;
            semantic(&stored, journal)?;
            DiagnosticSummaryMigrationCase::Candidate
        } else {
            return Err(ValidationError::rejected(Code::InvalidIdentity));
        };
        DiagnosticSummaryMigrationDecision::new(plan(journal, source.facts().sha256())?, case)
    }
    fn validate_recovered_stages(
        &self,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original_stage: &DiagnosticRecoverySource,
        candidate: &DiagnosticRecoverySource,
    ) -> Result<(), ValidationError> {
        let journal = planned(current, plan)?;
        original_copy(original_stage, &journal)?;
        let class = candidate.class();
        if !matches!(class, DiagnosticEntryClass::Stage | DiagnosticEntryClass::Summary) {
            return Err(ValidationError::rejected(Code::InvalidIdentity));
        }
        conversion(candidate, class, original_stage, &journal)
    }
    fn validate_archive_original(
        &self,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original_stage: &DiagnosticRecoverySource,
    ) -> Result<(), ValidationError> {
        let journal = planned(current, plan)?;
        original_copy(original_stage, &journal)?;
        let converted = original_semantics(original_stage.as_bytes(), &journal)?;
        if converted.len() as u64 != journal.candidate_stage().bytes()
            || digest(&converted) != journal.candidate_stage().sha256()
        {
            return Err(ValidationError::rejected(Code::InvalidIdentity));
        }
        Ok(())
    }
    fn validate_archive(
        &self,
        current: LogicalStoreOperationKey,
        source: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationArchiveDecision, ValidationError> {
        if !matches!(source.class(), DiagnosticEntryClass::Journal | DiagnosticEntryClass::Stage) {
            return Err(ValidationError::rejected(Code::InvalidIdentity));
        }
        whole(source)?;
        let state = decode_store_state(source.as_bytes()).map_err(codec_error)?;
        let IncidentStoreState::SummaryMigrationArchive(archive) = &state else {
            return Err(ValidationError::rejected(Code::InvalidJournal));
        };
        canonical(source, &state)?;
        historical(current, source, archive.migration())?;
        let historical_journal = encode_store_state(&IncidentStoreState::SummaryMigration(
            Box::new(archive.migration().clone()),
        ))
        .map_err(codec_error)?;
        DiagnosticSummaryMigrationArchiveDecision::new(
            plan(archive.migration(), digest(&historical_journal))?,
            source.facts().sha256(),
        )
    }
}
fn planned(
    current: LogicalStoreOperationKey,
    plan: &DiagnosticSummaryMigrationPlan,
) -> Result<SummaryMigrationJournal, ValidationError> {
    if current.owner().namespace != plan.historical_operation().owner().namespace {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    let journal = SummaryMigrationJournal::new(
        plan.historical_operation(),
        plan.incident(),
        plan.revision(),
        plan.payload_revision(),
        expected(plan.original_file())?,
        expected(plan.original_stage())?,
        expected(plan.candidate_stage())?,
    )
    .map_err(codec_error)?;
    let bytes =
        encode_store_state(&IncidentStoreState::SummaryMigration(Box::new(journal.clone())))
            .map_err(codec_error)?;
    if digest(&bytes) != plan.journal_sha256() {
        return Err(ValidationError::rejected(Code::InvalidJournal));
    }
    Ok(journal)
}
fn plan(
    journal: &SummaryMigrationJournal,
    sha256: [u8; 32],
) -> Result<DiagnosticSummaryMigrationPlan, ValidationError> {
    DiagnosticSummaryMigrationPlan::new(
        journal.operation(),
        journal.incident(),
        journal.revision(),
        journal.payload_revision(),
        recorded(journal.original_file())?,
        recorded(journal.original_stage())?,
        recorded(journal.candidate_stage())?,
        sha256,
    )
}
fn historical(
    current: LogicalStoreOperationKey,
    source: &DiagnosticRecoverySource,
    journal: &SummaryMigrationJournal,
) -> Result<(), ValidationError> {
    if current.owner().namespace != journal.operation().owner().namespace
        || source
            .object()
            .is_some_and(|object| object != DiagnosticObject::Journal(journal.operation()))
    {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn canonical(
    source: &DiagnosticRecoverySource,
    state: &IncidentStoreState,
) -> Result<(), ValidationError> {
    if encode_store_state(state).map_err(codec_error)?.as_slice() != source.as_bytes() {
        return Err(ValidationError::rejected(Code::InvalidJournal));
    }
    Ok(())
}
fn journal_source(source: &DiagnosticRecoverySource) -> Result<(), ValidationError> {
    if source.class() != DiagnosticEntryClass::Journal {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    whole(source)
}
fn summary_source(
    source: &DiagnosticRecoverySource,
    class: DiagnosticEntryClass,
    journal: &SummaryMigrationJournal,
) -> Result<(), ValidationError> {
    if source.class() != class
        || source
            .object()
            .is_some_and(|object| object != DiagnosticObject::Summary(journal.incident()))
    {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    whole(source)
}
fn original_copy(
    source: &DiagnosticRecoverySource,
    journal: &SummaryMigrationJournal,
) -> Result<(), ValidationError> {
    summary_source(source, DiagnosticEntryClass::Stage, journal)?;
    actual(source, journal.original_stage())?;
    original_semantics(source.as_bytes(), journal)?;
    Ok(())
}
fn original_semantics(
    bytes: &[u8],
    journal: &SummaryMigrationJournal,
) -> Result<Vec<u8>, ValidationError> {
    // Explicit migration also accepts schema2 for idempotence; the original MUST be1.
    let record: super::codec::Record<serde::de::IgnoredAny> = serde_json::from_slice(bytes)
        .map_err(|cause| codec_error(IncidentStoreError::from(cause)))?;
    if record.schema_version != 1 {
        return Err(ValidationError::rejected(Code::InvalidSchema));
    }
    let migrated = migrate_summary_v1(bytes).map_err(codec_error)?;
    semantic(&decode_summary(&migrated).map_err(codec_error)?, journal)?;
    Ok(migrated)
}
fn conversion(
    candidate: &DiagnosticRecoverySource,
    class: DiagnosticEntryClass,
    original: &DiagnosticRecoverySource,
    journal: &SummaryMigrationJournal,
) -> Result<(), ValidationError> {
    summary_source(candidate, class, journal)?;
    actual(candidate, journal.candidate_stage())?;
    let converted = original_semantics(original.as_bytes(), journal)?;
    if converted.as_slice() != candidate.as_bytes() {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    semantic(&decode_summary(candidate.as_bytes()).map_err(codec_error)?, journal)
}
fn semantic(
    stored: &StoredIncidentSummary,
    journal: &SummaryMigrationJournal,
) -> Result<(), ValidationError> {
    if stored.summary().seed().key() != journal.incident()
        || stored.summary().revision() != journal.revision()
        || stored.payload_revision() != journal.payload_revision()
    {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn whole(source: &DiagnosticRecoverySource) -> Result<(), ValidationError> {
    if source.as_bytes().len() > super::codec::STATE_BOUND
        || source.facts().bytes() != source.as_bytes().len() as u64
        || source.facts().sha256() != digest(source.as_bytes())
    {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn matches(actual: DiagnosticReadFacts, expected: IncidentFileFacts) -> bool {
    actual.file_identity() == expected.identity()
        && actual.sha256() == expected.sha256()
        && actual.bytes() == expected.bytes()
}
fn actual(
    source: &DiagnosticRecoverySource,
    expected: IncidentFileFacts,
) -> Result<(), ValidationError> {
    if !matches(source.facts(), expected) {
        return Err(ValidationError::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn recorded(facts: IncidentFileFacts) -> Result<RecordedDiagnosticFileFacts, ValidationError> {
    RecordedDiagnosticFileFacts::new(facts.identity(), facts.sha256(), facts.bytes())
        .map_err(|cause| ValidationError::caused(Code::InvalidIdentity, cause))
}
fn expected(facts: RecordedDiagnosticFileFacts) -> Result<IncidentFileFacts, ValidationError> {
    IncidentFileFacts::new(facts.file_identity(), facts.sha256(), facts.bytes())
        .map_err(codec_error)
}
fn codec_error(cause: IncidentStoreError) -> ValidationError {
    let code = if matches!(cause, IncidentStoreError::UnsupportedSchema(_)) {
        Code::InvalidSchema
    } else {
        Code::InvalidJournal
    };
    ValidationError::caused(code, cause)
}
