// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare actual protected sources while seeding only the external filesystem fixture.
use super::*;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
pub(super) type TestResult = Result<(), TestError>;
pub(super) struct Prepared {
    pub original: StoredIncidentSummary,
    pub candidate: StoredIncidentSummary,
    pub preparation: DiagnosticPayloadDiscardPreparation,
    pub journal: Vec<u8>,
    pub payload: DiagnosticObject,
    pub original_pin: DiagnosticReadLease,
    pub candidate_pin: DiagnosticStage,
}
pub(super) fn prepare(
    fixture: &DiagnosticStorageFixture,
    repository: &mut IncidentRepository,
    context: &JobContext,
    change_reporting: bool,
) -> Result<Prepared, TestError> {
    let bytes = b"exact retained redacted fault";
    let base = super::reporting_fixture::retained(bytes)?;
    let original = StoredIncidentSummary::new(
        IncidentSummary::new(base.seed().clone(), IncidentRevision::new(6)?, base.facts().clone())?,
        Some(base.revision()),
    )?;
    let original_bytes = encode_summary(&original)?;
    let key = original.summary().seed().key();
    let payload = DiagnosticObject::Payload(key, base.revision(), AttachmentKind::Fault);
    fixture.put(payload, bytes)?;
    fixture.put(DiagnosticObject::Summary(key), &original_bytes)?;
    repository.storage.inventory(&repository.operation, context)?;
    let mut original_pin = repository.storage.read(
        DiagnosticObject::Summary(key),
        65_536,
        &repository.operation,
        context,
    )?;
    let disposition = DiscardedPayload::new(PayloadDiscard::StoragePressure, bytes.len() as u64)?;
    let mut candidate = super::super::retention::candidate_summary(&original, disposition)?;
    if change_reporting {
        let summary = candidate.summary();
        let facts = summary.facts();
        candidate = StoredIncidentSummary::new(
            IncidentSummary::new(
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
            )?,
            None,
        )?;
    }
    let candidate_bytes = encode_summary(&candidate)?;
    let mut candidate_pin = repository.storage.stage(
        DiagnosticObject::Summary(key),
        candidate_bytes.len() as u64,
        &repository.operation,
        context,
    )?;
    candidate_pin.write_chunk(&candidate_bytes, context)?;
    candidate_pin.flush(context)?;
    let PayloadDisposition::Retained(attachments) = original.summary().facts().payload() else {
        return Err("fixture did not retain payload".into());
    };
    let selection = DiagnosticPayloadDiscardSelection::new(
        key,
        original.summary().revision(),
        base.revision(),
        attachments.clone(),
        disposition,
    )?;
    let preparation = repository.storage.prepare_payload_discard(
        &mut original_pin,
        &mut candidate_pin,
        selection,
        &repository.operation,
        context,
    )?;
    let victims = preparation
        .victims()
        .map(|victim| {
            let DiagnosticObject::Payload(_, _, kind) = victim.object() else {
                return Err(IncidentStoreError::RecoveryConflict);
            };
            let actual = victim.facts();
            PayloadDiscardVictimFacts::new(
                kind,
                actual.file_identity(),
                actual.sha256(),
                actual.bytes(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let file = |actual: DiagnosticReadFacts| {
        IncidentFileFacts::new(actual.file_identity(), actual.sha256(), actual.bytes())
    };
    let journal = encode_store_state(&IncidentStoreState::PayloadDiscard(Box::new(
        PayloadDiscardJournal::new(
            preparation.operation(),
            key,
            original.summary().revision(),
            base.revision(),
            file(preparation.original_file())?,
            file(preparation.candidate_stage())?,
            attachments.clone(),
            &victims,
            disposition,
        )?,
    )))?;
    Ok(Prepared { original, candidate, preparation, journal, payload, original_pin, candidate_pin })
}
