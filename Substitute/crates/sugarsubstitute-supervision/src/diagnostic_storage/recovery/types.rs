// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish private complete-read observations from inert codec decisions.
use super::*;
/// Compare the actual complete ledger to one exact durable journal generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticRecoveryLedgerCase {
    /// The original complete ledger remains current.
    Original,
    /// The checked candidate complete ledger already became current.
    Candidate,
}
/// Observe private whole-read bytes under the retained source's actual identity.
/// Construction is restricted to the storage owner after complete digest validation.
pub struct DiagnosticRecoverySource {
    pub(in crate::diagnostic_storage) bytes: Vec<u8>,
    pub(in crate::diagnostic_storage) facts: DiagnosticReadFacts,
    pub(in crate::diagnostic_storage) object: Option<DiagnosticObject>,
    pub(in crate::diagnostic_storage) class: DiagnosticEntryClass,
    pub(in crate::diagnostic_storage) name_digest: [u8; 32],
    pub(in crate::diagnostic_storage) origin: crate::diagnostic_storage::source::SourceOrigin,
    pub(in crate::diagnostic_storage) native: NativeId,
    pub(in crate::diagnostic_storage) _allocation: allocation::DiagnosticAllocationLoan,
}
impl DiagnosticRecoverySource {
    /// Borrow at most64KiB whole actual bytes; the source retains its native pin.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Observe actual retained file identity, digest and byte length.
    pub const fn facts(&self) -> DiagnosticReadFacts {
        self.facts
    }
    /// Observe a typed opened object; inventory candidates remain semantically unbound.
    pub const fn object(&self) -> Option<DiagnosticObject> {
        self.object
    }
    /// Observe the actual canonical candidate class, independent of codec assertions.
    pub const fn class(&self) -> DiagnosticEntryClass {
        self.class
    }
    /// Observe the actual canonical candidate leaf digest; never infer its full key.
    pub const fn name_digest(&self) -> [u8; 32] {
        self.name_digest
    }
}
/// Supply checked expected recovery data; this constructor grants no native permission.
pub struct DiagnosticRecoveryDecision {
    pub(super) plan: DiagnosticDispositionPlan,
    pub(super) ledger_case: DiagnosticRecoveryLedgerCase,
    pub(super) original_ledger_sha256: [u8; 32],
    pub(super) candidate_ledger_sha256: [u8; 32],
    pub(super) candidate_stage: RecordedDiagnosticFileFacts,
}
impl DiagnosticRecoveryDecision {
    /// Validate the frozen summary-eviction completion and distinct exact ledger hashes.
    /// # Errors
    /// Rejects other intents, non-summary victims, non-ledger completion, zero/equal
    /// hashes, an oversized candidate, or candidate full facts/digest differing from
    /// the durable journal completion.
    pub fn new(
        plan: DiagnosticDispositionPlan,
        ledger_case: DiagnosticRecoveryLedgerCase,
        original_ledger_sha256: [u8; 32],
        candidate_ledger_sha256: [u8; 32],
        candidate_stage: RecordedDiagnosticFileFacts,
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        if plan.intent != DispositionIntent::SummaryEviction
            || !matches!(plan.victim,DiagnosticObject::Summary(key) if key==plan.incident)
            || plan.completion != DiagnosticObject::StoreState
            || original_ledger_sha256 == [0; 32]
            || candidate_ledger_sha256 == [0; 32]
            || original_ledger_sha256 == candidate_ledger_sha256
            || candidate_ledger_sha256 != plan.completion_sha256
            || candidate_stage.sha256() != candidate_ledger_sha256
            || candidate_stage.bytes() > DIAGNOSTIC_IO_BYTES as u64
        {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        }
        Ok(Self {
            plan,
            ledger_case,
            original_ledger_sha256,
            candidate_ledger_sha256,
            candidate_stage,
        })
    }
    /// Observe inert complete candidate-stage facts decoded from the durable journal.
    pub const fn candidate_stage(&self) -> RecordedDiagnosticFileFacts {
        self.candidate_stage
    }
}
/// Validate bounded pure codec facts; filesystem effects remain exclusively with storage.
///
/// The composition-owned adapter must strictly decode full journal/ledger identities,
/// original and checked candidate counts/last facts/UTC, and exact canonical hashes.
/// Returned decisions are inert; storage independently checks actual native custody.
pub trait DiagnosticRecoveryValidator {
    /// Validate two complete protected reads under the current admitted recovery key.
    /// # Errors
    /// Preserve unknown schemas, corrupt/mismatched facts and checked-overflow causes.
    fn validate(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        current: LogicalStoreOperationKey,
        journal: &DiagnosticRecoverySource,
        ledger: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticRecoveryDecision, DiagnosticRecoveryValidationError>;
}
