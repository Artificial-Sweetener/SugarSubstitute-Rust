// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Record exact native preparation against actual committed completion facts.
use super::*;
use crate::incident::{
    CaptureDispositionCompletion, CaptureDispositionPlan, CaptureRecordedFileFacts,
    IncidentStoreError,
};
use sugarsubstitute_contracts::incident::{CaptureEvidence, IncidentSummary, PayloadDisposition};
use sugarsubstitute_supervision::diagnostic_storage::DiagnosticReadFacts;
impl Token {
    pub(crate) fn plan(
        &self,
        summary: &IncidentSummary,
        summary_file: DiagnosticReadFacts,
        payload_file: DiagnosticReadFacts,
    ) -> Result<CaptureDispositionPlan, IncidentStoreError> {
        let fields = self.fields.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        let prepared =
            fields.state.preparation.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        let (uuid, bytes) = self.dump().ok_or(IncidentStoreError::RecoveryConflict)?;
        if !self.matches(summary.seed().key(), summary.seed().build(), fields.operation)
            || summary.facts().capture() != CaptureEvidence::Finished(uuid)
            || payload_file.bytes() != bytes
            || payload_file.sha256() != prepared.source.sha256
            || summary_file.bytes() > 65_536
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let PayloadDisposition::Retained(attachments) = summary.facts().payload() else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        if !attachments.entries().any(|entry| {
            entry.kind() == sugarsubstitute_contracts::incident::AttachmentKind::Minidump
                && entry.retained_bytes() == bytes
                && *entry.sha256() == payload_file.sha256()
        }) {
            return Err(IncidentStoreError::WrongDigest);
        }
        let recorded = |file: &abi::FileFacts| {
            CaptureRecordedFileFacts::new(file.identity, file.sha256, file.bytes)
        };
        CaptureDispositionPlan::new(
            fields.operation,
            summary.seed().key(),
            summary.revision(),
            fields.operation.owner(),
            uuid,
            summary.seed().build().clone(),
            prepared.database_identity,
            prepared.metrics_identity,
            recorded(&prepared.source)?,
            recorded(&prepared.metadata)?,
            recorded(&prepared.original_stage.file)?,
            recorded(&prepared.candidate_stage.file)?,
            CaptureDispositionCompletion::Promoted {
                summary_file: CaptureRecordedFileFacts::new(
                    summary_file.file_identity(),
                    summary_file.sha256(),
                    summary_file.bytes(),
                )?,
                payload_file: CaptureRecordedFileFacts::new(
                    payload_file.file_identity(),
                    payload_file.sha256(),
                    payload_file.bytes(),
                )?,
            },
        )
    }
}
