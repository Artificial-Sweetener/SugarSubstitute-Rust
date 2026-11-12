// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replay only an actual canonical promotion before migration can change its committed Summary.
use super::*;
use crate::capture::CaptureRootSource;
use crate::incident::{CaptureDispositionCompletion, HistoricalCaptureRecovery};
/// Strict existing Journal decoder; validation supplies matching data, never native authority.
struct IncidentCapturePromotionValidator;
impl CapturePromotionValidator for IncidentCapturePromotionValidator {
    fn validate(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        source: &DiagnosticRecoverySource,
    ) -> Result<RecordedCapturePromotion, DiagnosticRecoveryValidationError> {
        use crate::incident::allocation::ValidationFailure as Failure;
        use DiagnosticRecoveryValidationCode as Code;
        crate::incident::allocation::validation(decode, || {
            let state = crate::incident::decode_store_state(source.as_bytes())
                .map_err(|cause| Failure::store(Code::InvalidJournal, cause))?;
            let IncidentStoreState::CapturePromotion(plan) = &state else {
                return Err(Failure::rejected(Code::InvalidJournal));
            };
            if source.class() != DiagnosticEntryClass::Journal
                || source.object().is_some_and(|object| {
                    object != DiagnosticObject::Journal(plan.historical_operation())
                })
                || source.facts().bytes() != source.as_bytes().len() as u64
                || source.facts().sha256()
                    != crate::incident::codec_ledger::digest(source.as_bytes())
                || crate::incident::encode_store_state(&state)
                    .map_err(|cause| Failure::store(Code::InvalidJournal, cause))?
                    .as_slice()
                    != source.as_bytes()
            {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            RecordedCapturePromotion::new(
                plan.historical_operation(),
                plan.capture_origin(),
                [plan.database_identity(), plan.metrics_identity()],
            )
            .map_err(Failure::from)
        })
    }
}
impl IncidentRepository {
    /// Settle at most one actual interrupted promotion before touching legacy Summary bytes.
    /// A current genuine Root source opens the exact recorded physical generation. The historical
    /// operation, native source, immutable Summary/payload and metadata stages are rechecked by
    /// the existing typed fence and v2 native recovery owner. Unknown/partial files remain intact.
    /// The supplied output must survive actual job terminal and checked report/repository cleanup.
    /// # Errors
    /// Missing source, changed canonical record/completion, unsupported disposition or uncertain
    /// native settlement preserves the same journal and durable partial output; no grant is forged.
    pub fn recover_one_capture_promotion(
        &mut self,
        source: Option<&CaptureRootSource>,
        output: &mut HistoricalCaptureRecovery,
        context: &JobContext,
    ) -> Result<bool, IncidentStoreError> {
        if output.operation.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let inventory = self.inventory(context)?;
        let mut selected = None;
        for entry in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Journal)
        {
            let (bytes, _, index) = self.read_candidate(entry, context)?;
            let mut decode = self.storage.recovery_decode(&self.operation, context)?;
            let promotion = crate::incident::allocation::decode(&mut decode, || {
                crate::incident::decode_store_state(bytes.as_bytes())
                    .map(|state| matches!(state, IncidentStoreState::CapturePromotion(_)))
            })?;
            drop(decode);
            drop(bytes);
            self.close_read(index, context)?;
            if promotion && selected.is_none_or(|known| entry.name_digest() < known) {
                selected = Some(entry.name_digest());
            }
        }
        let Some(selected) = selected else {
            return Ok(false);
        };
        let source = source.ok_or(IncidentStoreError::RecoveryConflict)?;
        source.origin_root(&self.operation, context)?;
        output.operation = Some(self.operation());
        let entry = inventory
            .entries()
            .find(|entry| {
                entry.class() == DiagnosticEntryClass::Journal && entry.name_digest() == selected
            })
            .ok_or(IncidentStoreError::RecoveryConflict)?;
        let (bytes, _, journal_index) = self.read_candidate(entry, context)?;
        let mut decode = self.storage.recovery_decode(&self.operation, context)?;
        output.retain_replay(crate::incident::allocation::decode(&mut decode, || {
            crate::incident::decode_store_state(bytes.as_bytes())
        })?)?;
        drop(decode);
        drop(bytes);
        let plan = output.replay.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        if plan.historical_operation() == self.operation() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.reads
            .get_mut(journal_index)?
            .bind_object(DiagnosticObject::Journal(plan.historical_operation()), context)?;
        source.prepare_promoted_finished(
            self.reads.get_mut(journal_index)?,
            &IncidentCapturePromotionValidator,
            &mut output.report,
            &mut self.storage,
            &self.operation,
            context,
        )?;
        let summary_index = self.reads.len();
        self.reads.push(self.storage.read(
            DiagnosticObject::Summary(plan.incident()),
            65536,
            &self.operation,
            context,
        )?)?;
        let bytes = self.reads.get_mut(summary_index)?.whole_source(context)?;
        let mut decode = self.storage.recovery_decode(&self.operation, context)?;
        let payload_revision = crate::incident::allocation::decode(&mut decode, || {
            let stored = crate::incident::decode_summary(bytes.as_bytes())?;
            let summary = stored.summary();
            if summary.seed().key() != plan.incident()
                || summary.seed().build() != plan.build()
                || summary.revision() != plan.incident_revision()
                || summary.facts().capture() != CaptureEvidence::Finished(plan.uuid())
            {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            stored.payload_revision().ok_or(IncidentStoreError::MissingPayload)
        })?;
        drop(decode);
        drop(bytes);
        let CaptureDispositionCompletion::Promoted { summary_file, payload_file } =
            plan.completion()
        else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        if !recorded_matches(self.reads.get_mut(summary_index)?.facts(), summary_file) {
            return Err(IncidentStoreError::ChangedDuplicate);
        }
        let payload_index = self.reads.len();
        self.reads.push(self.storage.read(
            DiagnosticObject::Payload(plan.incident(), payload_revision, AttachmentKind::Minidump),
            payload_file.bytes(),
            &self.operation,
            context,
        )?)?;
        if !recorded_matches(self.reads.get_mut(payload_index)?.facts(), payload_file) {
            return Err(IncidentStoreError::ChangedDuplicate);
        }
        let report = output.report.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        report.journal = Some(JournalState {
            record: IncidentStoreState::CapturePromotion(
                output.replay.take().ok_or(IncidentStoreError::RecoveryConflict)?,
            ),
            payload_revision,
            summary_index,
            payload_index,
            journal_index: Some(journal_index),
            stage_index: None,
            activation_started: false,
            write_uncertain: false,
            activation: None,
            intent: None,
            settled: false,
            journal_retired: false,
            payload_closed: false,
            summary_closed: false,
            failure: None,
        });
        let (summary, payload, journal) =
            self.reads.three_mut(summary_index, payload_index, journal_index)?;
        report.journal.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?.intent = Some(
            report
                .token
                .as_mut()
                .ok_or(IncidentStoreError::RecoveryConflict)?
                .adopt_capture_intent(
                    &mut self.storage,
                    journal,
                    summary,
                    Some(payload),
                    &self.operation,
                    context,
                )?,
        );
        self.settle_capture_journal(report, context)?;
        if !report.disposition_settled() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(true)
    }
}
fn recorded_matches(
    actual: DiagnosticReadFacts,
    expected: crate::incident::CaptureRecordedFileFacts,
) -> bool {
    actual.file_identity() == expected.identity()
        && actual.sha256() == expected.sha256()
        && actual.bytes() == expected.bytes()
}
