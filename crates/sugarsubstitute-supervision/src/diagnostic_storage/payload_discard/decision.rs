// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate inert exact generation expectations before retained boundary comparison.
use super::*;
impl DiagnosticPayloadDiscardDecision {
    /// Validate bounded distinct metadata and an exact canonical attachment set.
    /// This constructor supplies recorded data and grants no native permission.
    /// # Errors
    /// Rejects namespace, metadata, generation, order, digest or identity discrepancies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        selection: DiagnosticPayloadDiscardSelection,
        historical: LogicalStoreOperationKey,
        original_file: RecordedDiagnosticFileFacts,
        candidate_stage: RecordedDiagnosticFileFacts,
        victims: &[DiagnosticPayloadVictimExpectation],
        journal_sha256: [u8; 32],
        case: DiagnosticPayloadDiscardCase,
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        let invalid = || {
            DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            )
        };
        if historical.owner().namespace != selection.incident.subject().coordinator().namespace
            || original_file.bytes > DIAGNOSTIC_IO_BYTES as u64
            || candidate_stage.bytes > DIAGNOSTIC_IO_BYTES as u64
            || original_file.identity == candidate_stage.identity
            || original_file.sha256 == candidate_stage.sha256
            || journal_sha256 == [0; 32]
            || victims.len() > MAX_ATTACHMENTS
            || victims.len() != selection.attachments.entries().count()
        {
            return Err(invalid());
        }
        for (index, (victim, attachment)) in
            victims.iter().zip(selection.attachments.entries()).enumerate()
        {
            if victim.object
                != DiagnosticObject::Payload(
                    selection.incident,
                    selection.payload_revision,
                    attachment.kind(),
                )
                || victim.facts.sha256 != *attachment.sha256()
                || victim.facts.bytes != attachment.retained_bytes()
                || victim.facts.identity == original_file.identity
                || victim.facts.identity == candidate_stage.identity
                || victims[..index]
                    .iter()
                    .any(|prior| prior.facts.identity == victim.facts.identity)
            {
                return Err(invalid());
            }
        }
        let mut retained = Vec::new();
        retained.try_reserve_exact(MAX_ATTACHMENTS).map_err(|cause| {
            DiagnosticRecoveryValidationError::caused(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
                cause,
            )
        })?;
        retained.extend_from_slice(victims);
        Ok(Self {
            selection,
            historical,
            original: original_file,
            candidate: candidate_stage,
            victims: retained,
            journal_sha256,
            case,
        })
    }
}
