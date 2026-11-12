// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate recorded expected disposition data independently of actual observations.
use super::*;
use sugarsubstitute_contracts::incident::{IncidentKey, IncidentRevision};
/// Explain a repository-validated durable disposition, without authorizing it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispositionIntent {
    /// Explicitly remove an already durably described payload.
    PayloadDiscard,
    /// Remove one exact summary and commit its checked candidate ledger.
    SummaryEviction,
    /// Retire an exact validated quarantined object.
    QuarantineRetirement,
    /// Promote exact source bytes into a committed summary before source retirement.
    CapturePromotion,
}

/// Validate bounded recorded expectations; construction grants no native observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordedDiagnosticFileFacts {
    pub(super) identity: [u8; 24],
    pub(super) sha256: [u8; 32],
    pub(super) bytes: u64,
}
impl RecordedDiagnosticFileFacts {
    /// Validate recorded nonempty bytes and complete nonzero identity/digest.
    /// # Errors
    /// Rejects reserved identities/digests, empty or larger-than-256MiB expectations.
    pub fn new(
        identity: [u8; 24],
        sha256: [u8; 32],
        bytes: u64,
    ) -> Result<Self, DiagnosticStorageError> {
        if identity == [0; 24]
            || sha256 == [0; 32]
            || bytes == 0
            || bytes > sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES
        {
            return Err(DiagnosticStorageError::rejected(None, Code::InvalidIdentity));
        }
        Ok(Self { identity, sha256, bytes })
    }
    /// Observe historical expected file identity, without claiming retained custody.
    pub const fn file_identity(self) -> [u8; 24] {
        self.identity
    }
    /// Observe the recorded expected complete-content digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
    /// Observe the recorded expected nonempty byte length.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
    pub(super) fn matches(self, actual: DiagnosticReadFacts) -> bool {
        self.identity == actual.identity
            && self.sha256 == actual.sha256
            && self.bytes == actual.bytes
    }
}
/// Provide bounded exact intent data; construction never grants a deletion permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticDispositionPlan {
    pub(super) victim: DiagnosticObject,
    pub(super) incident: IncidentKey,
    pub(super) revision: IncidentRevision,
    pub(super) facts: RecordedDiagnosticFileFacts,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) intent: DispositionIntent,
    pub(super) journal_sha256: [u8; 32],
    pub(super) completion: DiagnosticObject,
    pub(super) completion_sha256: [u8; 32],
}
impl DiagnosticDispositionPlan {
    /// Validate exact victim, namespace, operation and canonical journal facts.
    ///
    /// The repository must additionally validate canonical journal bytes against
    /// these facts. Only actual checked journal activation can grant authority.
    /// # Errors
    /// Rejects foreign/mismatched objects, zero identity/digests or excessive size.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        victim: DiagnosticObject,
        incident: IncidentKey,
        revision: IncidentRevision,
        facts: DiagnosticReadFacts,
        operation: LogicalStoreOperationKey,
        intent: DispositionIntent,
        journal_sha256: [u8; 32],
        completion: DiagnosticObject,
        completion_sha256: [u8; 32],
    ) -> Result<Self, DiagnosticStorageError> {
        Self::from_recorded(
            victim,
            incident,
            revision,
            RecordedDiagnosticFileFacts::new(facts.identity, facts.sha256, facts.bytes)?,
            operation,
            intent,
            journal_sha256,
            completion,
            completion_sha256,
        )
    }
    /// Bind validated historical expected facts without claiming an actual observation.
    /// # Errors
    /// Rejects mismatched full identity, namespace, victim bounds or completion facts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_recorded(
        victim: DiagnosticObject,
        incident: IncidentKey,
        revision: IncidentRevision,
        facts: RecordedDiagnosticFileFacts,
        operation: LogicalStoreOperationKey,
        intent: DispositionIntent,
        journal_sha256: [u8; 32],
        completion: DiagnosticObject,
        completion_sha256: [u8; 32],
    ) -> Result<Self, DiagnosticStorageError> {
        let namespace = operation.owner().namespace;
        let matches_victim = match victim {
            DiagnosticObject::Seed(key) | DiagnosticObject::Summary(key) => key == incident,
            DiagnosticObject::Payload(key, actual, _) => key == incident && actual == revision,
            DiagnosticObject::StoreState | DiagnosticObject::Journal(_) => false,
        };
        let matches_completion = match completion {
            DiagnosticObject::StoreState => intent == DispositionIntent::SummaryEviction,
            DiagnosticObject::Summary(key) => key == incident,
            _ => false,
        };
        if !matches_victim
            || !matches_completion
            || incident.subject().coordinator().namespace != namespace
            || facts.bytes > victim.bound()
            || facts.identity == [0; 24]
            || facts.sha256 == [0; 32]
            || journal_sha256 == [0; 32]
            || completion_sha256 == [0; 32]
        {
            return Err(DiagnosticStorageError::rejected(
                Some(operation),
                DiagnosticStorageFailureCode::InvalidIdentity,
            ));
        }
        Ok(Self {
            victim,
            incident,
            revision,
            facts,
            operation,
            intent,
            journal_sha256,
            completion,
            completion_sha256,
        })
    }
    /// Observe the original complete operation, retaining historical ownership.
    pub const fn operation(self) -> LogicalStoreOperationKey {
        self.operation
    }
}
