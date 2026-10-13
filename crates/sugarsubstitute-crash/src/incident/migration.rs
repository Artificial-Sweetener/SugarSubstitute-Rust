// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact historical migration bytes supplied by the sole protected storage owner.
use super::allocation::ValidationFailure as Failure;
use super::{
    IncidentFileFacts, IncidentStoreError, IncidentStoreState, StoredIncidentSummary,
    SummaryMigrationJournal, codec_ledger::digest, decode_store_state, decode_summary,
    encode_store_state, migrate_summary_v1,
};
use sugarsubstitute_contracts::incident::LogicalStoreOperationKey;
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticEntryClass, DiagnosticObject, DiagnosticReadFacts, DiagnosticRecoveryDecodeLoan,
    DiagnosticRecoverySource, DiagnosticRecoveryValidationCode as Code,
    DiagnosticRecoveryValidationError as ValidationError,
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
        decode: &mut DiagnosticRecoveryDecodeLoan,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original: &DiagnosticRecoverySource,
        original_stage: &DiagnosticRecoverySource,
        candidate_stage: &DiagnosticRecoverySource,
    ) -> Result<(), ValidationError> {
        super::allocation::validation(decode, || {
            let journal = planned(current, plan)?;
            summary_source(original, DiagnosticEntryClass::Summary, &journal)?;
            actual(original, journal.original_file())?;
            original_copy(original_stage, &journal)?;
            if original.as_bytes() != original_stage.as_bytes() {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            conversion(candidate_stage, DiagnosticEntryClass::Stage, original_stage, &journal)
        })
    }
    fn validate_recovery(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        current: LogicalStoreOperationKey,
        source: &DiagnosticRecoverySource,
        summary: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationDecision, ValidationError> {
        super::allocation::validation(decode, || {
            journal_source(source)?;
            let state = decode_store_state(source.as_bytes()).map_err(codec_error)?;
            let IncidentStoreState::SummaryMigration(journal) = &state else {
                return Err(Failure::rejected(Code::InvalidJournal));
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
                return Err(Failure::rejected(Code::InvalidIdentity));
            };
            DiagnosticSummaryMigrationDecision::new(plan(journal, source.facts().sha256())?, case)
                .map_err(Failure::from)
        })
    }
    fn validate_recovered_stages(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original_stage: &DiagnosticRecoverySource,
        candidate: &DiagnosticRecoverySource,
    ) -> Result<(), ValidationError> {
        super::allocation::validation(decode, || {
            let journal = planned(current, plan)?;
            original_copy(original_stage, &journal)?;
            let class = candidate.class();
            if !matches!(class, DiagnosticEntryClass::Stage | DiagnosticEntryClass::Summary) {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            conversion(candidate, class, original_stage, &journal)
        })
    }
    fn validate_archive_original(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original_stage: &DiagnosticRecoverySource,
    ) -> Result<(), ValidationError> {
        super::allocation::validation(decode, || {
            let journal = planned(current, plan)?;
            original_copy(original_stage, &journal)?;
            let converted = original_semantics(original_stage.as_bytes(), &journal)?;
            if converted.len() as u64 != journal.candidate_stage().bytes()
                || digest(&converted) != journal.candidate_stage().sha256()
            {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            Ok(())
        })
    }
    fn validate_archive(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        current: LogicalStoreOperationKey,
        source: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationArchiveDecision, ValidationError> {
        super::allocation::validation(decode, || {
            if !matches!(
                source.class(),
                DiagnosticEntryClass::Journal | DiagnosticEntryClass::Stage
            ) {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            whole(source)?;
            let state = decode_store_state(source.as_bytes()).map_err(codec_error)?;
            let IncidentStoreState::SummaryMigrationArchive(archive) = &state else {
                return Err(Failure::rejected(Code::InvalidJournal));
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
            .map_err(Failure::from)
        })
    }
}
fn planned(
    current: LogicalStoreOperationKey,
    plan: &DiagnosticSummaryMigrationPlan,
) -> Result<SummaryMigrationJournal, Failure> {
    if current.owner().namespace != plan.historical_operation().owner().namespace {
        return Err(Failure::rejected(Code::InvalidIdentity));
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
        return Err(Failure::rejected(Code::InvalidJournal));
    }
    Ok(journal)
}
fn plan(
    journal: &SummaryMigrationJournal,
    sha256: [u8; 32],
) -> Result<DiagnosticSummaryMigrationPlan, Failure> {
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
    .map_err(Failure::from)
}
fn historical(
    current: LogicalStoreOperationKey,
    source: &DiagnosticRecoverySource,
    journal: &SummaryMigrationJournal,
) -> Result<(), Failure> {
    if current.owner().namespace != journal.operation().owner().namespace
        || source
            .object()
            .is_some_and(|object| object != DiagnosticObject::Journal(journal.operation()))
    {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn canonical(source: &DiagnosticRecoverySource, state: &IncidentStoreState) -> Result<(), Failure> {
    if encode_store_state(state).map_err(codec_error)?.as_slice() != source.as_bytes() {
        return Err(Failure::rejected(Code::InvalidJournal));
    }
    Ok(())
}
fn journal_source(source: &DiagnosticRecoverySource) -> Result<(), Failure> {
    if source.class() != DiagnosticEntryClass::Journal {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    whole(source)
}
fn summary_source(
    source: &DiagnosticRecoverySource,
    class: DiagnosticEntryClass,
    journal: &SummaryMigrationJournal,
) -> Result<(), Failure> {
    if source.class() != class
        || source
            .object()
            .is_some_and(|object| object != DiagnosticObject::Summary(journal.incident()))
    {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    whole(source)
}
fn original_copy(
    source: &DiagnosticRecoverySource,
    journal: &SummaryMigrationJournal,
) -> Result<(), Failure> {
    summary_source(source, DiagnosticEntryClass::Stage, journal)?;
    actual(source, journal.original_stage())?;
    original_semantics(source.as_bytes(), journal)?;
    Ok(())
}
fn original_semantics(bytes: &[u8], journal: &SummaryMigrationJournal) -> Result<Vec<u8>, Failure> {
    // Explicit migration also accepts schema2 for idempotence; the original MUST be1.
    let record: super::codec::Record<serde::de::IgnoredAny> = super::codec_parse::parse(bytes)
        .map_err(|cause| codec_error(IncidentStoreError::from(cause)))?;
    if record.schema_version != 1 {
        return Err(Failure::rejected(Code::InvalidSchema));
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
) -> Result<(), Failure> {
    summary_source(candidate, class, journal)?;
    actual(candidate, journal.candidate_stage())?;
    let converted = original_semantics(original.as_bytes(), journal)?;
    if converted.as_slice() != candidate.as_bytes() {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    semantic(&decode_summary(candidate.as_bytes()).map_err(codec_error)?, journal)
}
fn semantic(
    stored: &StoredIncidentSummary,
    journal: &SummaryMigrationJournal,
) -> Result<(), Failure> {
    if stored.summary().seed().key() != journal.incident()
        || stored.summary().revision() != journal.revision()
        || stored.payload_revision() != journal.payload_revision()
    {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn whole(source: &DiagnosticRecoverySource) -> Result<(), Failure> {
    if source.as_bytes().len() > super::codec::STATE_BOUND
        || source.facts().bytes() != source.as_bytes().len() as u64
        || source.facts().sha256() != digest(source.as_bytes())
    {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn matches(actual: DiagnosticReadFacts, expected: IncidentFileFacts) -> bool {
    actual.file_identity() == expected.identity()
        && actual.sha256() == expected.sha256()
        && actual.bytes() == expected.bytes()
}
fn actual(source: &DiagnosticRecoverySource, expected: IncidentFileFacts) -> Result<(), Failure> {
    if !matches(source.facts(), expected) {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    Ok(())
}
fn recorded(facts: IncidentFileFacts) -> Result<RecordedDiagnosticFileFacts, Failure> {
    RecordedDiagnosticFileFacts::new(facts.identity(), facts.sha256(), facts.bytes())
        .map_err(|cause| Failure::storage(Code::InvalidIdentity, cause))
}
fn expected(facts: RecordedDiagnosticFileFacts) -> Result<IncidentFileFacts, Failure> {
    IncidentFileFacts::new(facts.file_identity(), facts.sha256(), facts.bytes())
        .map_err(codec_error)
}
fn codec_error(cause: IncidentStoreError) -> Failure {
    let code = if matches!(cause, IncidentStoreError::UnsupportedSchema(_)) {
        Code::InvalidSchema
    } else {
        Code::InvalidJournal
    };
    Failure::store(code, cause)
}
