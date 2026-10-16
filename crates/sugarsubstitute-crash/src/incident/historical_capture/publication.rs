// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original seed and actual native exception facts before immutable payload publication.
use super::*;
impl IncidentRepository {
    pub(super) fn recover_historical_capture_once(
        &mut self,
        source: &CaptureRootSource,
        checked: &CheckedRecoveryCaptureOrigin,
        observed_at: IncidentUtc,
        output: &mut HistoricalCaptureRecovery,
        context: &JobContext,
    ) -> Result<HistoricalCaptureRecoveryState, IncidentStoreError> {
        let key = output.key.ok_or(IncidentStoreError::RecoveryConflict)?;
        if let Some((summary, _)) = self.find_summary(key, context)? {
            if summary.summary().seed().build() != checked.build() {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            return Ok(HistoricalCaptureRecoveryState::AlreadyPublished);
        }
        self.historical_seed(key, checked.build(), observed_at, output, context)?;
        let Some(seed) = output.seed.as_ref() else {
            return Ok(HistoricalCaptureRecoveryState::SeedUnavailable);
        };
        source.prepare_historical_recovery_finished(
            checked,
            &mut output.report,
            &mut self.storage,
            &self.operation,
            context,
        )?;
        let report = output.report.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let capture = report.open(key, seed.build(), None, context)?;
        let CaptureEvidence::Finished(_) = capture else {
            return Ok(HistoricalCaptureRecoveryState::Unavailable(capture));
        };
        output.frame = Some(report.exception_frame(context).map_err(|cause| {
            output.frame_error = Some(cause);
            IncidentStoreError::RecoveryConflict
        })?);
        let frame = output.frame.ok_or(IncidentStoreError::RecoveryConflict)?;
        if frame.captured_at() < seed.created() || frame.captured_at() > observed_at {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let bytes = report.dump().ok_or(IncidentStoreError::MissingPayload)?.1;
        report.prepare_disposition(context)?;
        let headroom = report.publication_headroom().ok_or(IncidentStoreError::RecoveryConflict)?;
        let preflight = self.preflight_publication(
            seed.clone(),
            ExpectedSummary::Absent,
            IncidentPublicationDemand::new(bytes, 1, 1, headroom)?,
            observed_at,
            context,
        )?;
        output.ticket = Some(self.begin_publish(preflight, self.operation().id(), context)?);
        let ticket = output.ticket.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let attachment = self.append_finished(ticket, report, context)?;
        let frame = output.frame.ok_or(IncidentStoreError::RecoveryConflict)?;
        let summary = IncidentSummary::new(
            seed.clone(),
            seed.revision().next()?,
            SummaryFacts::new(
                RetentionFacts::new(frame.captured_at(), None)?,
                ExitAttribution::CaptureOnly,
                FailureEvidence::NativeFault { code: frame.code(), address: frame.address() },
                capture,
                PayloadDisposition::Retained(AttachmentInventory::new(&[attachment])?),
                ReportDisposition::new(0, None)?,
                SymbolOutcome::NotRequested,
            ),
        )?;
        output.committed = Some(self.commit(ticket, summary, context)?);
        self.publish_capture_journal(
            report,
            output.committed.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?,
            context,
        )?;
        self.settle_capture_journal(report, context)?;
        if !report.disposition_settled() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(HistoricalCaptureRecoveryState::Published)
    }
    fn historical_seed(
        &mut self,
        key: IncidentKey,
        build: &BuildIdentity,
        now: IncidentUtc,
        output: &mut HistoricalCaptureRecovery,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let inventory = self.inventory(context)?;
        for candidate in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Seed)
        {
            let (bytes, _, index) = self.read_candidate(candidate, context)?;
            let mut decode = self.storage.recovery_decode(&self.operation, context)?;
            let seed = super::super::allocation::decode(&mut decode, || {
                super::super::decode_seed(bytes.as_bytes())
            })?;
            drop(decode);
            drop(bytes);
            self.reads.get_mut(index)?.bind_object(DiagnosticObject::Seed(seed.key()), context)?;
            if seed.key() == key {
                if output.seed.is_some() || seed.build() != build || seed.created() > now {
                    return Err(IncidentStoreError::RecoveryConflict);
                }
                output.seed = Some(seed);
            }
            self.close_read(index, context)?;
        }
        Ok(())
    }
}
