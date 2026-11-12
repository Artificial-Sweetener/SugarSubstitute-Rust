// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish collected host facts and capture payloads through the same finite repository operation.
use super::*;

/// Facts selected after the real capture boundary has finished its bounded observation.
/// Native report custody remains in CollectionOperation; these values grant no capture authority.
pub(super) struct CollectionEvidence {
    pub(super) seed: IncidentSeed,
    pub(super) scope: IncidentOperationScope,
    pub(super) occurred: IncidentUtc,
    pub(super) capture: CaptureEvidence,
    pub(super) dump_bytes: u64,
    pub(super) host_failure: Option<HostFailureFact>,
}

impl CollectionOperation {
    /// Commit the exact coordinator-owned summary and independently settle a finished dump.
    /// A host-failure appendix can be retained when native capture is unavailable.
    /// # Errors
    /// Preserve partial publication, genuine policy rejection and native disposition custody.
    #[expect(clippy::result_large_err, reason = "Keep funded CollectionOperation cause inline")]
    pub(super) fn publish_evidence(
        &mut self,
        evidence: CollectionEvidence,
        coordinator: &mut IncidentCoordinator,
        effects: &mut [Option<IncidentEffect>; MAX_INCIDENT_EFFECTS],
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let repository = self.repository.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let host_failure = evidence.host_failure.as_ref().map(RedactedTextChunk::from_host_failure);
        let dump_bytes = evidence.dump_bytes;
        let bytes = dump_bytes
            .checked_add(host_failure.as_ref().map_or(0, |value| value.as_bytes().len() as u64))
            .ok_or(IncidentStoreError::Capacity)?;
        let headroom = if dump_bytes == 0 {
            131072
        } else {
            self.stage = CollectionStage::PrepareDisposition;
            let report = self.report.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            report.prepare_disposition(context)?;
            report.publication_headroom().ok_or(IncidentStoreError::RecoveryConflict)?
        };
        self.stage = CollectionStage::Preflight;
        let preflight = repository.preflight_publication(
            evidence.seed,
            ExpectedSummary::Absent,
            IncidentPublicationDemand::new(bytes, usize::from(bytes != 0), 1, headroom)?,
            evidence.occurred,
            context,
        )?;
        self.stage = CollectionStage::BeginPublish;
        self.ticket =
            Some(repository.begin_publish(preflight, evidence.scope.operation().id(), context)?);
        let ticket = self.ticket.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let mut attachments = Vec::new();
        attachments.try_reserve_exact(2).map_err(|_| IncidentStoreError::Capacity)?;
        if dump_bytes != 0 {
            self.stage = CollectionStage::CopyPayload;
            let report = self.report.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            attachments.push(repository.append_finished(ticket, report, context)?);
        }
        if let Some(chunk) = host_failure {
            attachments.push(repository.append_text(
                ticket,
                AttachmentKind::Fault,
                chunk,
                context,
            )?);
        }
        let payload = if attachments.is_empty() {
            PayloadDisposition::Unavailable
        } else {
            PayloadDisposition::Retained(AttachmentInventory::new(&attachments)?)
        };
        self.stage = CollectionStage::ObserveEvidence;
        let produced = coordinator
            .observe(IncidentServiceObservation::EvidenceCollected {
                scope: evidence.scope,
                evidence: CollectedIncidentEvidence::new(
                    evidence.occurred,
                    evidence.capture,
                    payload,
                    SymbolOutcome::NotRequested,
                ),
            })
            .map_err(|cause| {
                self.decision = Some(cause);
                IncidentStoreError::RecoveryConflict
            })?;
        for (slot, effect) in effects.iter_mut().zip(produced) {
            *slot = Some(effect);
        }
        let slot = effects
            .iter_mut()
            .find(|slot| slot.is_some())
            .ok_or(IncidentStoreError::RecoveryConflict)?;
        let Some(IncidentEffect::PublishSummary { scope, summary }) = slot.as_ref() else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        if *scope != evidence.scope {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        // The coordinator owns the exact expected content; storage independently verifies all files.
        self.stage = CollectionStage::Commit;
        self.committed = Some(repository.commit(ticket, summary.clone(), context)?);
        *slot = None;
        let committed = self.committed.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        if dump_bytes != 0 {
            let report = self.report.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            self.stage = CollectionStage::PublishJournal;
            repository.publish_capture_journal(report, committed, context)?;
            self.stage = CollectionStage::SettleJournal;
            repository.settle_capture_journal(report, context)?;
            self.source_settled = report.disposition_settled();
        }
        if effects.iter().any(Option::is_some) {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.stage = CollectionStage::ObserveCommitted;
        let produced = coordinator
            .observe(IncidentServiceObservation::SummaryCommitted {
                scope: evidence.scope,
                summary: committed.summary().clone(),
            })
            .map_err(|cause| {
                self.decision = Some(cause);
                IncidentStoreError::RecoveryConflict
            })?;
        for (slot, effect) in effects.iter_mut().zip(produced) {
            *slot = Some(effect);
        }
        self.stage = CollectionStage::Committed;
        Ok(())
    }
}

#[cfg(test)]
#[path = "publication/tests.rs"]
mod tests;
