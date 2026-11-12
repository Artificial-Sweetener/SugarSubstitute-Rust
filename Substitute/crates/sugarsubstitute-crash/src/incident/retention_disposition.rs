// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle one exact payload group through its actual retained source and Journal owners.
use super::{
    IncidentFileFacts, IncidentPayloadDiscardValidator, IncidentRepository, IncidentStoreError,
    IncidentStoreState, PayloadDiscardJournal, PayloadDiscardVictimFacts,
};
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
use sugarsubstitute_supervision::diagnostic_storage::*;
impl IncidentRepository {
    pub(super) fn discard_retained_payload(
        &mut self,
        stored: super::allocation::RetainedSummary,
        expected: DiagnosticReadFacts,
        reason: PayloadDiscard,
        context: &JobContext,
    ) -> Result<u64, IncidentStoreError> {
        let summary = stored.summary();
        let key = summary.seed().key();
        let PayloadDisposition::Retained(attachments) = summary.facts().payload() else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        let payload_revision =
            stored.payload_revision().ok_or(IncidentStoreError::MissingPayload)?;
        let disposition = DiscardedPayload::new(reason, attachments.total_bytes())?;
        let candidate_custody = self.storage.recovery_record(&self.operation, context)?;
        let candidate = super::allocation::RetainedSummary::new(
            super::retention::candidate_summary(&stored, disposition)?,
            candidate_custody,
        );
        let candidate_bytes = self.encode_summary(&candidate, context)?;
        let original_index = self.reads.len();
        self.reads.push(self.storage.read(
            DiagnosticObject::Summary(key),
            MAX_SUMMARY_BYTES as u64,
            &self.operation,
            context,
        )?)?;
        if self.reads.get_mut(original_index)?.facts() != expected {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let stage_index = self.stages.len();
        self.stages.push(self.storage.stage(
            DiagnosticObject::Summary(key),
            candidate_bytes.as_bytes().len() as u64,
            &self.operation,
            context,
        )?)?;
        self.stages.get_mut(stage_index)?.write_chunk(candidate_bytes.as_bytes(), context)?;
        self.stages.get_mut(stage_index)?.flush(context)?;
        let phase = self.storage.recovery_decode(&self.operation, context)?;
        let selection = DiagnosticPayloadDiscardSelection::new(
            key,
            summary.revision(),
            payload_revision,
            attachments.clone(),
            disposition,
        )?;
        let mut preparation = self.storage.prepare_payload_discard(
            self.reads.get_mut(original_index)?,
            self.stages.get_mut(stage_index)?,
            selection,
            &self.operation,
            context,
        )?;
        drop(phase);
        let mut phase = self.storage.recovery_decode(&self.operation, context)?;
        let mut journal_bytes = self.storage.record_buffer(&self.operation, context)?;
        super::allocation::decode(&mut phase, || {
            let victims = preparation
                .victims()
                .map(|victim| {
                    let DiagnosticObject::Payload(_, _, kind) = victim.object() else {
                        return Err(IncidentStoreError::RecoveryConflict);
                    };
                    let facts = victim.facts();
                    PayloadDiscardVictimFacts::new(
                        kind,
                        facts.file_identity(),
                        facts.sha256(),
                        facts.bytes(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let recorded = |actual: DiagnosticReadFacts| {
                IncidentFileFacts::new(actual.file_identity(), actual.sha256(), actual.bytes())
            };
            let journal = IncidentStoreState::PayloadDiscard(Box::new(PayloadDiscardJournal::new(
                self.operation(),
                key,
                summary.revision(),
                payload_revision,
                recorded(preparation.original_file())?,
                recorded(preparation.candidate_stage())?,
                attachments.clone(),
                &victims,
                disposition,
            )?));
            super::codec_ledger::write_store_state(&journal, &mut journal_bytes)
        })?;
        drop(phase);
        let mut stage = self.storage.seal_payload_discard(
            &mut preparation,
            journal_bytes.as_bytes(),
            &IncidentPayloadDiscardValidator,
            &self.operation,
            context,
        )?;
        let mut permit = self.storage.activate_payload_discard(
            &mut preparation,
            &mut stage,
            &self.operation,
            context,
        )?;
        let completion =
            self.storage.complete_payload_discard(&mut permit, &self.operation, context)?;
        self.storage.finish_payload_discard(&mut permit, &completion, &self.operation, context)?;
        self.reads.remove(original_index)?;
        self.stages.remove(stage_index)?;
        Ok(completion.reclaimed_bytes())
    }
}
