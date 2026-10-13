// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact journal and ledger generations from storage-owned complete read observations.
use super::allocation::ValidationFailure as Failure;
use super::{
    IncidentFileFacts, IncidentStoreError, IncidentStoreState, codec_ledger::digest,
    decode_store_state, encode_store_state,
};
use sugarsubstitute_contracts::incident::LogicalStoreOperationKey;
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticDispositionPlan, DiagnosticEntryClass, DiagnosticObject, DiagnosticReadFacts,
    DiagnosticRecoveryDecision, DiagnosticRecoveryDecodeLoan, DiagnosticRecoveryLedgerCase,
    DiagnosticRecoverySource, DiagnosticRecoveryValidationCode as Code,
    DiagnosticRecoveryValidationError as ValidationError, DiagnosticRecoveryValidator,
    DispositionIntent, RecordedDiagnosticFileFacts,
};

/// Strict summary-eviction semantic consumer; only storage can provide its read observations.
/// Decisions contain recorded data and grant no deletion, stage or native authority.
pub struct IncidentRecoveryValidator;
impl DiagnosticRecoveryValidator for IncidentRecoveryValidator {
    fn validate(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        current: LogicalStoreOperationKey,
        journal_source: &DiagnosticRecoverySource,
        ledger_source: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticRecoveryDecision, ValidationError> {
        super::allocation::validation(decode, || {
            if journal_source.class() != DiagnosticEntryClass::Journal
                || ledger_source.class() != DiagnosticEntryClass::StoreState
                || ledger_source
                    .object()
                    .is_some_and(|object| object != DiagnosticObject::StoreState)
            {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            let state = decode_store_state(journal_source.as_bytes()).map_err(codec_error)?;
            let IncidentStoreState::SummaryEviction(journal) = &state else {
                return Err(Failure::rejected(Code::InvalidJournal));
            };
            if current.owner().namespace != journal.operation().owner().namespace
                || journal_source
                    .object()
                    .is_some_and(|object| object != DiagnosticObject::Journal(journal.operation()))
                || encode_store_state(&state).map_err(codec_error)?.as_slice()
                    != journal_source.as_bytes()
                || !whole_read_matches(journal_source)
                || !whole_read_matches(ledger_source)
            {
                return Err(Failure::rejected(Code::InvalidJournal));
            }
            let ledger = decode_store_state(ledger_source.as_bytes()).map_err(codec_error)?;
            let IncidentStoreState::Ledger(ledger) = ledger else {
                return Err(Failure::rejected(Code::LedgerMismatch));
            };
            let ledger_case = if ledger == journal.original()
                && actual_matches(ledger_source.facts(), journal.original_file())
            {
                DiagnosticRecoveryLedgerCase::Original
            } else if ledger == journal.candidate()
                && actual_matches(ledger_source.facts(), journal.candidate_stage())
            {
                DiagnosticRecoveryLedgerCase::Candidate
            } else {
                return Err(Failure::rejected(Code::LedgerMismatch));
            };
            let key = journal.incident().map_err(codec_error)?;
            let victim = recorded(journal.victim())?;
            let candidate_stage = recorded(journal.candidate_stage())?;
            let plan = DiagnosticDispositionPlan::from_recorded(
                DiagnosticObject::Summary(key),
                key,
                journal.revision(),
                victim,
                journal.operation(),
                DispositionIntent::SummaryEviction,
                journal_source.facts().sha256(),
                DiagnosticObject::StoreState,
                journal.candidate_sha256(),
            )
            .map_err(|cause| Failure::storage(Code::InvalidIdentity, cause))?;
            DiagnosticRecoveryDecision::new(
                plan,
                ledger_case,
                journal.original_file().sha256(),
                journal.candidate_sha256(),
                candidate_stage,
            )
            .map_err(Failure::from)
        })
    }
}
fn recorded(facts: IncidentFileFacts) -> Result<RecordedDiagnosticFileFacts, Failure> {
    RecordedDiagnosticFileFacts::new(facts.identity(), facts.sha256(), facts.bytes())
        .map_err(|cause| Failure::storage(Code::InvalidIdentity, cause))
}
fn actual_matches(actual: DiagnosticReadFacts, expected: IncidentFileFacts) -> bool {
    actual.file_identity() == expected.identity()
        && actual.sha256() == expected.sha256()
        && actual.bytes() == expected.bytes()
}
fn whole_read_matches(source: &DiagnosticRecoverySource) -> bool {
    source.facts().bytes() == source.as_bytes().len() as u64
        && source.facts().sha256() == digest(source.as_bytes())
}
fn codec_error(cause: IncidentStoreError) -> Failure {
    let code = match &cause {
        IncidentStoreError::UnsupportedSchema(_) => Code::InvalidSchema,
        IncidentStoreError::Facts(
            sugarsubstitute_contracts::incident::IncidentError::EvictionCountExhausted,
        ) => Code::ArithmeticOverflow,
        _ => Code::InvalidJournal,
    };
    Failure::store(code, cause)
}
