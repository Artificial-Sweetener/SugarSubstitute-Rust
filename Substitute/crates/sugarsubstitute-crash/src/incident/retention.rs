// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact whole-source payload reclamation semantics before protected native effects.
use super::allocation::ValidationFailure as Failure;
use super::{
    IncidentFileFacts, IncidentStoreError, IncidentStoreState, PayloadDiscardJournal,
    StoredIncidentSummary, codec_ledger::digest, decode_store_state, decode_summary,
    encode_store_state, encode_summary,
};
use sugarsubstitute_contracts::incident::*;
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticEntryClass, DiagnosticObject, DiagnosticPayloadDecisionAllocationLoan,
    DiagnosticPayloadDiscardCase as Case, DiagnosticPayloadDiscardDecision as Decision,
    DiagnosticPayloadDiscardSelection as Selection, DiagnosticPayloadDiscardValidator,
    DiagnosticPayloadVictimExpectation, DiagnosticPayloadVictimObservation, DiagnosticReadFacts,
    DiagnosticRecoveryDecodeLoan, DiagnosticRecoverySource as Source,
    DiagnosticRecoveryValidationCode as Code, DiagnosticRecoveryValidationError as ValidationError,
    RecordedDiagnosticFileFacts,
};

/// Semantic consumer of actual protected Summary, Journal, candidate and victim observations.
/// Returned decisions are inert matching data; only the storage owner grants effects.
pub struct IncidentPayloadDiscardValidator;
impl DiagnosticPayloadDiscardValidator for IncidentPayloadDiscardValidator {
    fn validate_preparation(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        allocation: DiagnosticPayloadDecisionAllocationLoan,
        current: LogicalStoreOperationKey,
        selection: &Selection,
        journal_bytes: &[u8],
        original: &Source,
        candidate: &Source,
        victims: &[DiagnosticPayloadVictimObservation],
    ) -> Result<Decision, ValidationError> {
        super::allocation::validation(decode, || {
            let journal = decode_journal(current, journal_bytes)?;
            if journal.operation() != current
                || selection.incident() != journal.incident()
                || selection.summary_revision() != journal.original_revision()
                || selection.payload_revision() != journal.payload_revision()
                || selection.attachments() != journal.original_payload()
                || selection.disposition() != journal.disposition()
                || !actual_matches(original.facts(), journal.original_file())
                || !actual_matches(candidate.facts(), journal.candidate_stage())
                || victims.len() != journal.victims().count()
            {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            let original = summary_source(original, &journal, DiagnosticEntryClass::Summary)?;
            let candidate = summary_source(candidate, &journal, DiagnosticEntryClass::Stage)?;
            check_original(&original, &journal)?;
            let expected =
                candidate_summary(&original, journal.disposition()).map_err(codec_error)?;
            if candidate != expected {
                return Err(Failure::rejected(Code::LedgerMismatch));
            }
            for (actual, expected) in victims.iter().zip(journal.victims()) {
                if actual.object()
                    != DiagnosticObject::Payload(
                        journal.incident(),
                        journal.payload_revision(),
                        expected.kind(),
                    )
                    || actual.facts().file_identity() != expected.identity()
                    || actual.facts().sha256() != expected.sha256()
                    || actual.facts().bytes() != expected.bytes()
                {
                    return Err(Failure::rejected(Code::InvalidIdentity));
                }
            }
            decision(allocation, &journal, digest(journal_bytes), Case::Original)
        })
    }
    fn validate_recovery(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        allocation: DiagnosticPayloadDecisionAllocationLoan,
        current: LogicalStoreOperationKey,
        journal_source: &Source,
        summary: &Source,
    ) -> Result<Decision, ValidationError> {
        super::allocation::validation(decode, || {
            let journal = decode_journal(current, journal_source.as_bytes())?;
            if journal_source.class() != DiagnosticEntryClass::Journal
                || journal_source
                    .object()
                    .is_some_and(|object| object != DiagnosticObject::Journal(journal.operation()))
                || !whole(journal_source)
            {
                return Err(Failure::rejected(Code::InvalidJournal));
            }
            let stored = summary_source(summary, &journal, DiagnosticEntryClass::Summary)?;
            let case = if actual_matches(summary.facts(), journal.original_file()) {
                check_original(&stored, &journal)?;
                let candidate =
                    candidate_summary(&stored, journal.disposition()).map_err(codec_error)?;
                check_bytes(&candidate, journal.candidate_stage())?;
                Case::Original
            } else if actual_matches(summary.facts(), journal.candidate_stage()) {
                if stored.summary().revision()
                    != journal
                        .original_revision()
                        .next()
                        .map_err(|cause| Failure::facts(Code::ArithmeticOverflow, cause))?
                    || stored.payload_revision().is_some()
                    || stored.summary().facts().payload()
                        != &PayloadDisposition::Discarded(journal.disposition())
                {
                    return Err(Failure::rejected(Code::LedgerMismatch));
                }
                let original = replace_payload(
                    &stored,
                    journal.original_revision(),
                    PayloadDisposition::Retained(journal.original_payload().clone()),
                    Some(journal.payload_revision()),
                )
                .map_err(codec_error)?;
                check_original(&original, &journal)?;
                check_bytes(&original, journal.original_file())?;
                Case::Candidate
            } else {
                return Err(Failure::rejected(Code::LedgerMismatch));
            };
            decision(allocation, &journal, journal_source.facts().sha256(), case)
        })
    }
}
fn decode_journal(
    current: LogicalStoreOperationKey,
    bytes: &[u8],
) -> Result<PayloadDiscardJournal, Failure> {
    let state = decode_store_state(bytes).map_err(codec_error)?;
    let IncidentStoreState::PayloadDiscard(journal) = &state else {
        return Err(Failure::rejected(Code::InvalidJournal));
    };
    if current.owner().namespace != journal.operation().owner().namespace
        || encode_store_state(&state).map_err(codec_error)?.as_slice() != bytes
    {
        return Err(Failure::rejected(Code::InvalidJournal));
    }
    Ok(journal.as_ref().clone())
}
fn summary_source(
    source: &Source,
    journal: &PayloadDiscardJournal,
    class: DiagnosticEntryClass,
) -> Result<StoredIncidentSummary, Failure> {
    if source.class() != class
        || !whole(source)
        || source
            .object()
            .is_some_and(|object| object != DiagnosticObject::Summary(journal.incident()))
    {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    let stored = decode_summary(source.as_bytes()).map_err(codec_error)?;
    if stored.summary().seed().key() != journal.incident()
        || encode_summary(&stored).map_err(codec_error)?.as_slice() != source.as_bytes()
    {
        return Err(Failure::rejected(Code::InvalidIdentity));
    }
    Ok(stored)
}
fn check_original(
    stored: &StoredIncidentSummary,
    journal: &PayloadDiscardJournal,
) -> Result<(), Failure> {
    if stored.summary().revision() != journal.original_revision()
        || stored.payload_revision() != Some(journal.payload_revision())
        || stored.summary().facts().payload()
            != &PayloadDisposition::Retained(journal.original_payload().clone())
    {
        return Err(Failure::rejected(Code::LedgerMismatch));
    }
    Ok(())
}
fn check_bytes(stored: &StoredIncidentSummary, facts: IncidentFileFacts) -> Result<(), Failure> {
    let bytes = encode_summary(stored).map_err(codec_error)?;
    if bytes.len() as u64 != facts.bytes() || digest(&bytes) != facts.sha256() {
        return Err(Failure::rejected(Code::LedgerMismatch));
    }
    Ok(())
}
fn decision(
    allocation: DiagnosticPayloadDecisionAllocationLoan,
    journal: &PayloadDiscardJournal,
    sha: [u8; 32],
    case: Case,
) -> Result<Decision, Failure> {
    let selection = Selection::new(
        journal.incident(),
        journal.original_revision(),
        journal.payload_revision(),
        journal.original_payload().clone(),
        journal.disposition(),
    )
    .map_err(|cause| Failure::storage(Code::InvalidIdentity, cause))?;
    let victims = journal
        .victims()
        .map(|victim| {
            let facts = RecordedDiagnosticFileFacts::new(
                victim.identity(),
                victim.sha256(),
                victim.bytes(),
            )
            .map_err(|cause| Failure::storage(Code::InvalidIdentity, cause))?;
            DiagnosticPayloadVictimExpectation::new(
                DiagnosticObject::Payload(
                    journal.incident(),
                    journal.payload_revision(),
                    victim.kind(),
                ),
                facts,
            )
            .map_err(Failure::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Decision::new(
        allocation,
        selection,
        journal.operation(),
        recorded(journal.original_file())?,
        recorded(journal.candidate_stage())?,
        &victims,
        sha,
        case,
    )
    .map_err(Failure::from)
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
fn whole(source: &Source) -> bool {
    source.facts().bytes() == source.as_bytes().len() as u64
        && source.facts().sha256() == digest(source.as_bytes())
}
fn codec_error(cause: IncidentStoreError) -> Failure {
    let code = match &cause {
        IncidentStoreError::UnsupportedSchema(_) => Code::InvalidSchema,
        IncidentStoreError::Facts(IncidentError::RevisionExhausted) => Code::ArithmeticOverflow,
        _ => Code::InvalidJournal,
    };
    Failure::store(code, cause)
}
pub(super) fn candidate_summary(
    stored: &StoredIncidentSummary,
    disposition: DiscardedPayload,
) -> Result<StoredIncidentSummary, IncidentStoreError> {
    replace_payload(
        stored,
        stored.summary().revision().next()?,
        PayloadDisposition::Discarded(disposition),
        None,
    )
}
fn replace_payload(
    stored: &StoredIncidentSummary,
    revision: IncidentRevision,
    payload: PayloadDisposition,
    payload_revision: Option<IncidentRevision>,
) -> Result<StoredIncidentSummary, IncidentStoreError> {
    let summary = stored.summary();
    let facts = summary.facts();
    StoredIncidentSummary::new(
        IncidentSummary::new(
            summary.seed().clone(),
            revision,
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                payload,
                facts.reporting(),
                facts.symbols().clone(),
            ),
        )?,
        payload_revision,
    )
}
