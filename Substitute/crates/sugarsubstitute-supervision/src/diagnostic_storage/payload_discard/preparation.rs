// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence actual Summary and candidate slots before opening the private payload group.
use super::*;
use sha2::{Digest, Sha256};
impl DiagnosticStorage {
    /// Retain the selected Summary, candidate and exact DELETE-capable payloads.
    /// Runs on an admitted worker under original context and writer exclusion.
    /// # Errors
    /// Saturation, export pins, identity discrepancies and partial effects preserve custody.
    pub fn prepare_payload_discard(
        &mut self,
        original: &mut DiagnosticReadLease,
        candidate: &mut DiagnosticStage,
        selection: DiagnosticPayloadDiscardSelection,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPayloadDiscardPreparation, DiagnosticStorageError> {
        pin(&self.shared, &original.shared, original.operation, operation)?;
        pin(&self.shared, &candidate.shared, candidate.operation, operation)?;
        let preparation_allocation = {
            let mut state = self.check(operation, context)?;
            allocation::payload_preparation(&state.data_mut()?.allocation, operation.key)?
        };
        let id = {
            let mut state = self.check(operation, context)?;
            stage::mutation(&mut state, operation.key)?;
            capacity(&mut state, operation.key)?;
            if state.data_mut()?.deletion.is_some()
                || selection.incident.subject().coordinator().namespace
                    != operation.key.owner().namespace
            {
                return Err(error(operation.key, Code::Busy));
            }
            let object = DiagnosticObject::Summary(selection.incident);
            let index = stage_index(&mut state, candidate.id, operation.key)?;
            if state.data_mut()?.stages[index].object != object || state.data_mut()?.stages[index].role.is_some()
                || state.data_mut()?.stages[index].flushed.is_none()
                || state.data_mut()?.reads.iter().any(|read| read.id != original.id && matches!(read.object, Some(DiagnosticObject::Summary(key) | DiagnosticObject::Payload(key, ..)) if key == selection.incident))
                || state.data_mut()?.reads.iter().find(|read| read.id == original.id).is_none_or(|read| read.object != Some(object) || read.facts != Some(original.facts))
            { return Err(error(operation.key, Code::InvalidIdentity)); }
            let intent_allocation =
                allocation::payload_intent(&state.data_mut()?.allocation, operation.key)?;
            let reservation =
                allocation::reservation(&state.data_mut()?.allocation, Some(operation.key))?;
            let mut victims = Vec::new();
            victims
                .try_reserve_exact(MAX_ATTACHMENTS)
                .map_err(|cause| reservation.failed(cause))?;
            for attachment in selection.attachments.entries() {
                victims.push(RetainedVictim::new(DiagnosticObject::Payload(
                    selection.incident,
                    selection.payload_revision,
                    attachment.kind(),
                )));
            }
            let id = next_lease(&mut state, operation.key)?;
            state.data_mut()?.stages[index].role = Some(stage::StageRole::PayloadCandidate);
            state.data_mut()?.payload = Some(intent_allocation.payload_intent(PayloadIntent {
                id,
                subject_digest:
                    crate::durable_storage::protected::record_file::diagnostic_subject_digest(
                        selection.incident,
                    ),
                reserved_files: MAX_ATTACHMENTS + 2,
                selection: Some(selection.clone()),
                original_read: Some(original.id),
                pending_journal_read: None,
                candidate_stage: Some(candidate.id),
                journal_stage: None,
                journal: None,
                victims,
                validating: true,
                decision: None,
                journal_bytes: None,
                journal_active: None,
                candidate_active: None,
            }));
            id
        };
        let original_source = whole(&self.shared, original, operation, context)?;
        let candidate_source = stage_by_id(self, operation, candidate.id, context)?;
        let mut state = self.check(operation, context)?;
        retained_source(&mut state, &original_source, operation.key, context)?;
        retained_source(&mut state, &candidate_source, operation.key, context)?;
        if candidate_source.class != DiagnosticEntryClass::Stage
            || original_source.facts.identity == candidate_source.facts.identity
            || original_source.facts.sha256 == candidate_source.facts.sha256
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        for (index, attachment) in selection.attachments.entries().enumerate() {
            check(&mut state, operation.key, context, false)?;
            let object = intent(&mut state, id, operation.key)?.victims[index].object;
            let (native_id, actual) = state
                .call_native(|port| port.open(object, object.bound(), context))?
                .map_err(|cause| native(operation.key, Code::Read, cause))?;
            let victims = &mut intent(&mut state, id, operation.key)?.victims;
            victims[index].native = Some(native_id);
            victims[index].facts = Some(actual);
            intent(&mut state, id, operation.key)?.reserved_files -= 1;
            let victims = &intent(&mut state, id, operation.key)?.victims;
            if native_id == 0
                || actual.identity == [0; 24]
                || actual.identity == original_source.facts.identity
                || actual.identity == candidate_source.facts.identity
                || actual.sha256 != *attachment.sha256()
                || actual.bytes != attachment.retained_bytes()
                || victims[..index]
                    .iter()
                    .any(|prior| prior.facts.is_some_and(|facts| facts.identity == actual.identity))
            {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            check(&mut state, operation.key, context, false)?;
        }
        let reservation =
            allocation::reservation(&state.data_mut()?.allocation, Some(operation.key))?;
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(MAX_ATTACHMENTS)
            .map_err(|cause| reservation.failed(cause))?;
        let retained = intent(&mut state, id, operation.key)?;
        for victim in &retained.victims {
            observations.push(DiagnosticPayloadVictimObservation {
                object: victim.object,
                facts: victim.facts.ok_or_else(|| error(operation.key, Code::Closed))?,
            });
        }
        retained.validating = false;
        retained.reserved_files = 2;
        Ok(DiagnosticPayloadDiscardPreparation {
            shared: self.shared.clone(),
            current: operation.key,
            id,
            selection,
            original: original_source.facts,
            candidate: candidate_source.facts,
            victims: observations,
            _allocation: preparation_allocation,
        })
    }
    /// Validate actual sources outside the mutex and stage exact canonical Journal bytes.
    /// # Errors
    /// Changed retries, callback/context discrepancies and partial stages remain retained.
    pub fn seal_payload_discard(
        &mut self,
        preparation: &mut DiagnosticPayloadDiscardPreparation,
        canonical_journal: &[u8],
        validator: &dyn DiagnosticPayloadDiscardValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPayloadDiscardStage, DiagnosticStorageError> {
        pin(&self.shared, &preparation.shared, preparation.current, operation)?;
        if canonical_journal.is_empty() || canonical_journal.len() > DIAGNOSTIC_IO_BYTES {
            return Err(error(operation.key, Code::Capacity));
        }
        let (original, candidate) = {
            let mut state = self.check(operation, context)?;
            let retained = intent(&mut state, preparation.id, operation.key)?;
            if let Some(bytes) = &retained.journal_bytes {
                if bytes != canonical_journal {
                    return Err(error(operation.key, Code::ChangedIdentity));
                }
                if retained.journal_stage.is_some() && retained.decision.is_some() {
                    return Ok(DiagnosticPayloadDiscardStage {
                        shared: self.shared.clone(),
                        current: operation.key,
                        id: preparation.id,
                    });
                }
                return Err(error(operation.key, Code::Closed));
            }
            if retained.validating || retained.selection.as_ref() != Some(&preparation.selection) {
                return Err(error(operation.key, Code::Busy));
            }
            retained.validating = true;
            (
                required(retained.original_read, operation.key)?,
                required(retained.candidate_stage, operation.key)?,
            )
        };
        let original = read_by_id(self, operation, original, context)?;
        let candidate = stage_by_id(self, operation, candidate, context)?;
        let destination = self.payload_decision(operation, context)?;
        let decision = allocation::decode_callback(self, operation, context, |decode| {
            validator.validate_preparation(
                decode,
                destination,
                operation.key,
                &preparation.selection,
                canonical_journal,
                &original,
                &candidate,
                &preparation.victims,
            )
        })?;
        let mut state = self.check(operation, context)?;
        retained_source(&mut state, &original, operation.key, context)?;
        retained_source(&mut state, &candidate, operation.key, context)?;
        revalidate_victims(&mut state, preparation.id, operation.key, context)?;
        if !decision._allocation.matches(operation)
            || decision.selection != preparation.selection
            || decision.historical != operation.key
            || decision.case != DiagnosticPayloadDiscardCase::Original
            || !decision.original.matches(original.facts)
            || !decision.candidate.matches(candidate.facts)
            || decision.journal_sha256 != <[u8; 32]>::from(Sha256::digest(canonical_journal))
            || decision.victims.iter().zip(&preparation.victims).any(|(expected, actual)| {
                expected.object != actual.object || !expected.facts.matches(actual.facts)
            })
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let reservation =
            allocation::reservation(&state.data_mut()?.allocation, Some(operation.key))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(canonical_journal.len())
            .map_err(|cause| reservation.failed(cause))?;
        bytes.extend_from_slice(canonical_journal);
        let retained = intent(&mut state, preparation.id, operation.key)?;
        retained.journal_bytes = Some(bytes);
        retained.decision = Some(decision);
        retained.validating = false;
        let stage_id = reserve_stage(
            &mut state,
            DiagnosticObject::Journal(operation.key),
            stage::StageRole::PayloadJournal,
            DIAGNOSTIC_IO_BYTES as u64,
            operation.key,
        )?;
        intent(&mut state, preparation.id, operation.key)?.journal_stage = Some(stage_id);
        create_stage(&mut state, stage_id, operation.key, context)?;
        intent(&mut state, preparation.id, operation.key)?.reserved_files = 1;
        append_stage(&mut state, stage_id, canonical_journal, operation.key, context)?;
        let facts = flush_stage(&mut state, stage_id, operation.key, context)?;
        if facts.sha256 != <[u8; 32]>::from(Sha256::digest(canonical_journal)) {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        Ok(DiagnosticPayloadDiscardStage {
            shared: self.shared.clone(),
            current: operation.key,
            id: preparation.id,
        })
    }
}
