// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound inert selection facts separately from actual retained transaction custody.
use super::*;

/// Select an exact immutable payload generation for bounded retention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticPayloadDiscardSelection {
    pub(super) incident: IncidentKey,
    pub(super) summary_revision: IncidentRevision,
    pub(super) payload_revision: IncidentRevision,
    pub(super) attachments: AttachmentInventory,
    pub(super) disposition: DiscardedPayload,
}
impl DiagnosticPayloadDiscardSelection {
    /// Validate the next Summary revision, original attachment order and reason.
    /// Construction grants no filesystem authority.
    /// # Errors
    /// Rejects exhausted revisions, non-retention reasons or inconsistent byte facts.
    pub fn new(
        incident: IncidentKey,
        summary_revision: IncidentRevision,
        payload_revision: IncidentRevision,
        attachments: AttachmentInventory,
        disposition: DiscardedPayload,
    ) -> Result<Self, DiagnosticStorageError> {
        if attachments.entries().any(|attachment| *attachment.sha256() == [0; 32]) {
            return Err(DiagnosticStorageError::rejected(None, Code::InvalidIdentity));
        }
        if summary_revision.next().is_err()
            || payload_revision.get() > summary_revision.get()
            || attachments.total_bytes() != disposition.source_bytes()
            || !matches!(
                disposition.reason(),
                PayloadDiscard::AcknowledgedRetention | PayloadDiscard::StoragePressure
            )
        {
            return Err(DiagnosticStorageError::rejected(None, Code::InvalidIdentity));
        }
        Ok(Self { incident, summary_revision, payload_revision, attachments, disposition })
    }
    /// Observe the complete failed incident identity.
    pub const fn incident(&self) -> IncidentKey {
        self.incident
    }
    /// Observe the exact original Summary revision.
    pub const fn summary_revision(&self) -> IncidentRevision {
        self.summary_revision
    }
    /// Observe the immutable attachment publication revision.
    pub const fn payload_revision(&self) -> IncidentRevision {
        self.payload_revision
    }
    /// Borrow original retained attachment and omission facts.
    pub const fn attachments(&self) -> &AttachmentInventory {
        &self.attachments
    }
    /// Observe the selected bounded retention reason and bytes.
    pub const fn disposition(&self) -> DiscardedPayload {
        self.disposition
    }
}
/// Observe one actually retained canonical payload without exposing its handle.
pub struct DiagnosticPayloadVictimObservation {
    pub(super) object: DiagnosticObject,
    pub(super) facts: DiagnosticReadFacts,
}
impl DiagnosticPayloadVictimObservation {
    /// Observe the exact immutable typed attachment object.
    pub const fn object(&self) -> DiagnosticObject {
        self.object
    }
    /// Observe actual whole-file identity, SHA and retained bytes.
    pub const fn facts(&self) -> DiagnosticReadFacts {
        self.facts
    }
}
/// Retain a private preparation; dropping it leaves all owner custody intact.
pub struct DiagnosticPayloadDiscardPreparation {
    pub(super) shared: Shared,
    pub(super) current: LogicalStoreOperationKey,
    pub(super) id: u64,
    pub(super) selection: DiagnosticPayloadDiscardSelection,
    pub(super) original: DiagnosticReadFacts,
    pub(super) candidate: DiagnosticReadFacts,
    pub(super) victims: Vec<DiagnosticPayloadVictimObservation>,
}
impl DiagnosticPayloadDiscardPreparation {
    /// Borrow the exact selected generation and retention facts.
    pub const fn selection(&self) -> &DiagnosticPayloadDiscardSelection {
        &self.selection
    }
    /// Observe the actual current operation without granting replacement authority.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.current
    }
    /// Observe the actual untouched Summary file.
    pub const fn original_file(&self) -> DiagnosticReadFacts {
        self.original
    }
    /// Observe the actual flushed candidate Summary stage.
    pub const fn candidate_stage(&self) -> DiagnosticReadFacts {
        self.candidate
    }
    /// Borrow the actual private payload observations in canonical order.
    pub fn victims(&self) -> impl Iterator<Item = &DiagnosticPayloadVictimObservation> {
        self.victims.iter()
    }
}
/// Retain an exact immutable Journal stage for this preparation.
pub struct DiagnosticPayloadDiscardStage {
    pub(super) shared: Shared,
    pub(super) current: LogicalStoreOperationKey,
    pub(super) id: u64,
}
/// Retain actual committed Journal custody and its selected group.
pub struct DiagnosticPayloadDiscardPermit {
    pub(super) shared: Shared,
    pub(super) current: LogicalStoreOperationKey,
    pub(super) id: u64,
}
/// Observe a checked candidate Summary and fully settled exact payload group.
pub struct DiagnosticPayloadDiscardCompletion {
    pub(super) id: u64,
    pub(super) summary: DiagnosticActivation,
    pub(super) bytes: u64,
}
impl DiagnosticPayloadDiscardCompletion {
    /// Borrow the actual candidate Summary activation observation.
    pub const fn summary(&self) -> &DiagnosticActivation {
        &self.summary
    }
    /// Observe bytes whose exact handles closed and protected absence was checked.
    pub const fn reclaimed_bytes(&self) -> u64 {
        self.bytes
    }
    /// Observe one fully settled payload group.
    pub const fn reclaimed_payload_groups(&self) -> u64 {
        1
    }
}
/// Classify exact current Summary facts without granting native effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticPayloadDiscardCase {
    /// Current Summary is the untouched original; all victims must exist.
    Original,
    /// Current Summary is the exact committed candidate; victims may be settled.
    Candidate,
}
/// Carry bounded recorded payload expectations without claiming observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticPayloadVictimExpectation {
    pub(super) object: DiagnosticObject,
    pub(super) facts: RecordedDiagnosticFileFacts,
}
impl DiagnosticPayloadVictimExpectation {
    /// Validate an allowlisted immutable payload object and its physical bound.
    /// # Errors
    /// Rejects metadata objects or payload bytes exceeding their purpose bound.
    pub fn new(
        object: DiagnosticObject,
        facts: RecordedDiagnosticFileFacts,
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        if !matches!(object, DiagnosticObject::Payload(..)) || facts.bytes > object.bound() {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        }
        Ok(Self { object, facts })
    }
}
/// Carry codec-validated expectations; only retained owner checks mint a permit.
pub struct DiagnosticPayloadDiscardDecision {
    pub(super) selection: DiagnosticPayloadDiscardSelection,
    pub(super) historical: LogicalStoreOperationKey,
    pub(super) original: RecordedDiagnosticFileFacts,
    pub(super) candidate: RecordedDiagnosticFileFacts,
    pub(super) victims: Vec<DiagnosticPayloadVictimExpectation>,
    pub(super) journal_sha256: [u8; 32],
    pub(super) case: DiagnosticPayloadDiscardCase,
}
/// Validate strict canonical bytes over actual retained source observations.
/// Callbacks run outside the owner mutex; returned data grants no native authority.
pub trait DiagnosticPayloadDiscardValidator {
    /// Prove exact candidate delta and canonical Journal for the selected sources.
    /// # Errors
    /// Rejects schema, identity, revision, attachment and unchanged-field discrepancies.
    fn validate_preparation(
        &self,
        current: LogicalStoreOperationKey,
        selection: &DiagnosticPayloadDiscardSelection,
        journal_bytes: &[u8],
        original: &DiagnosticRecoverySource,
        candidate: &DiagnosticRecoverySource,
        victims: &[DiagnosticPayloadVictimObservation],
    ) -> Result<DiagnosticPayloadDiscardDecision, DiagnosticRecoveryValidationError>;
    /// Prove Original or Candidate from an actual historical Journal and Summary.
    /// # Errors
    /// Rejects unknown tags, changed current Summary or noncanonical reconstruction.
    fn validate_recovery(
        &self,
        current: LogicalStoreOperationKey,
        journal: &DiagnosticRecoverySource,
        summary: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticPayloadDiscardDecision, DiagnosticRecoveryValidationError>;
}
