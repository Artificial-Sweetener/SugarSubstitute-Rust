// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish inert migration expectations from actual retained storage authority.
use super::*;
use sugarsubstitute_contracts::incident::{IncidentKey, IncidentRevision};

/// Maximum preserved migration archives; admission includes the pending reservation.
pub const DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND: usize = 128;
/// Classify one exact current Summary generation without authorizing a filesystem effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSummaryMigrationCase {
    /// The untouched legacy Summary is current.
    Original,
    /// The recorded schema2 candidate already became current.
    Candidate,
}
/// Bound recorded expectations; native observations are constructed separately by storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticSummaryMigrationPlan {
    pub(super) historical: LogicalStoreOperationKey,
    pub(super) incident: IncidentKey,
    pub(super) revision: IncidentRevision,
    pub(super) payload_revision: Option<IncidentRevision>,
    pub(super) original_file: RecordedDiagnosticFileFacts,
    pub(super) original_stage: RecordedDiagnosticFileFacts,
    pub(super) candidate_stage: RecordedDiagnosticFileFacts,
    pub(super) journal_sha256: [u8; 32],
}
impl DiagnosticSummaryMigrationPlan {
    /// Validate complete bounded recorded facts for the fixed schema1 to schema2 migration.
    /// # Errors
    /// Rejects foreign namespace, equal/reserved identities or digests, inconsistent
    /// original-copy bytes, empty/oversized streams and a zero journal digest.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        historical: LogicalStoreOperationKey,
        incident: IncidentKey,
        revision: IncidentRevision,
        payload_revision: Option<IncidentRevision>,
        original_file: RecordedDiagnosticFileFacts,
        original_stage: RecordedDiagnosticFileFacts,
        candidate_stage: RecordedDiagnosticFileFacts,
        journal_sha256: [u8; 32],
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        if historical.owner().namespace != incident.subject().coordinator().namespace
            || original_file.sha256 != original_stage.sha256
            || original_file.bytes != original_stage.bytes
            || original_file.identity == original_stage.identity
            || original_file.identity == candidate_stage.identity
            || original_stage.identity == candidate_stage.identity
            || original_stage.sha256 == candidate_stage.sha256
            || [original_file, original_stage, candidate_stage]
                .iter()
                .any(|facts| facts.bytes == 0 || facts.bytes > DIAGNOSTIC_IO_BYTES as u64)
            || journal_sha256 == [0; 32]
        {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        }
        Ok(Self {
            historical,
            incident,
            revision,
            payload_revision,
            original_file,
            original_stage,
            candidate_stage,
            journal_sha256,
        })
    }
    /// Observe the full historical key; it never authorizes the current operation.
    pub const fn historical_operation(&self) -> LogicalStoreOperationKey {
        self.historical
    }
    /// Observe the complete incident identity recorded in the migration.
    pub const fn incident(&self) -> IncidentKey {
        self.incident
    }
    /// Observe the immutable legacy Summary revision.
    pub const fn revision(&self) -> IncidentRevision {
        self.revision
    }
    /// Observe the immutable payload publication revision, or its explicit absence.
    pub const fn payload_revision(&self) -> Option<IncidentRevision> {
        self.payload_revision
    }
    /// Observe expected untouched active legacy Summary facts.
    pub const fn original_file(&self) -> RecordedDiagnosticFileFacts {
        self.original_file
    }
    /// Observe expected preserved original-copy facts.
    pub const fn original_stage(&self) -> RecordedDiagnosticFileFacts {
        self.original_stage
    }
    /// Observe expected exact schema2 candidate facts.
    pub const fn candidate_stage(&self) -> RecordedDiagnosticFileFacts {
        self.candidate_stage
    }
    /// Observe the expected complete canonical intent digest.
    pub const fn journal_sha256(&self) -> [u8; 32] {
        self.journal_sha256
    }
}
/// Carry a pure decoder's checked current-generation classification.
pub struct DiagnosticSummaryMigrationDecision {
    pub(super) plan: DiagnosticSummaryMigrationPlan,
    pub(super) case: DiagnosticSummaryMigrationCase,
}
impl DiagnosticSummaryMigrationDecision {
    /// Associate inert bounded data with an explicit original/candidate observation.
    /// # Errors
    /// The already validated plan contains all bounds; construction has no native effects.
    pub fn new(
        plan: DiagnosticSummaryMigrationPlan,
        case: DiagnosticSummaryMigrationCase,
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        Ok(Self { plan, case })
    }
}
/// Carry a pure decoder's expected completed Archive, without mutation permission.
pub struct DiagnosticSummaryMigrationArchiveDecision {
    pub(super) plan: DiagnosticSummaryMigrationPlan,
    pub(super) archive_sha256: [u8; 32],
}
impl DiagnosticSummaryMigrationArchiveDecision {
    /// Validate the complete recorded Archive digest independently of native custody.
    /// # Errors
    /// Rejects a zero digest or an Archive identical to its original intent bytes.
    pub fn new(
        plan: DiagnosticSummaryMigrationPlan,
        archive_sha256: [u8; 32],
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        if archive_sha256 == [0; 32] || archive_sha256 == plan.journal_sha256 {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        }
        Ok(Self { plan, archive_sha256 })
    }
}
/// Validate actual bounded bytes without receiving scheduling or native privileges.
///
/// Storage retains every source and invokes callbacks outside its state mutex,
/// then revalidates exact source custody and original execution context.
pub trait DiagnosticSummaryMigrationValidator {
    /// Prove strict legacy1 to canonical2 byte equality and complete domain facts.
    /// # Errors
    /// Preserves strict decoding, conversion, identity and payload-reference causes.
    fn validate_preparation(
        &self,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original: &DiagnosticRecoverySource,
        original_stage: &DiagnosticRecoverySource,
        candidate_stage: &DiagnosticRecoverySource,
    ) -> Result<(), DiagnosticRecoveryValidationError>;
    /// Decode the actual intent and classify the exact current Summary generation.
    /// # Errors
    /// Rejects unknown/third generations and noncanonical historical intent bytes.
    fn validate_recovery(
        &self,
        current: LogicalStoreOperationKey,
        journal: &DiagnosticRecoverySource,
        summary: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationDecision, DiagnosticRecoveryValidationError>;
    /// Prove conversion over the actual preserved original and recovered candidate.
    /// # Errors
    /// Rejects any schema/domain/revision/payload-reference or byte discrepancy.
    fn validate_recovered_stages(
        &self,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original_stage: &DiagnosticRecoverySource,
        candidate: &DiagnosticRecoverySource,
    ) -> Result<(), DiagnosticRecoveryValidationError>;
    /// Validate archived facts against strict legacy1 and derived canonical2 SHA/length.
    /// Current Summary is not required and no Summary authority is returned.
    /// # Errors
    /// Rejects inconsistent archived original or derived candidate facts.
    fn validate_archive_original(
        &self,
        current: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original_stage: &DiagnosticRecoverySource,
    ) -> Result<(), DiagnosticRecoveryValidationError>;
    /// Decode actual canonical Archive bytes into bounded inert matching data.
    /// # Errors
    /// Rejects unknown schemas, noncanonical bytes or inconsistent complete facts.
    fn validate_archive(
        &self,
        current: LogicalStoreOperationKey,
        archive: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationArchiveDecision, DiagnosticRecoveryValidationError>;
}

/// Pin actual internally created original/candidate custody; Drop never retires it.
pub struct DiagnosticSummaryMigrationPreparation {
    pub(super) shared: Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) id: u64,
    pub(super) original: DiagnosticReadFacts,
    pub(super) copy: DiagnosticStageFacts,
}
impl DiagnosticSummaryMigrationPreparation {
    /// Observe actual complete original Summary facts without mutation authority.
    pub fn original_file(&self) -> DiagnosticReadFacts {
        self.original
    }
    /// Observe actual flushed untouched original-copy facts.
    pub fn original_stage(&self) -> DiagnosticStageFacts {
        self.copy
    }
}
/// Pin the sealed intent Journal stage; no ordinary mutable stage escapes.
pub struct DiagnosticSummaryMigrationJournalStage {
    pub(super) shared: Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) id: u64,
}
/// Pin actual durable intent and preserved original-copy custody for one operation.
pub struct DiagnosticSummaryMigrationPermit {
    pub(super) shared: Shared,
    pub(super) current: LogicalStoreOperationKey,
    pub(super) id: u64,
    pub(super) plan: DiagnosticSummaryMigrationPlan,
}
impl DiagnosticSummaryMigrationPermit {
    /// Observe full historical provenance without reopening that operation.
    pub const fn historical_operation(&self) -> LogicalStoreOperationKey {
        self.plan.historical
    }
    /// Observe the actual currently admitted operation.
    pub const fn current_operation(&self) -> LogicalStoreOperationKey {
        self.current
    }
    /// Borrow immutable matching expectations, not native permissions.
    pub fn plan(&self) -> &DiagnosticSummaryMigrationPlan {
        &self.plan
    }
}
/// Observe actual Archive activation/readback/closure after original-copy checked closure.
pub struct DiagnosticSummaryMigrationArchiveCommit {
    pub(super) current: LogicalStoreOperationKey,
    pub(super) historical: LogicalStoreOperationKey,
    pub(super) facts: DiagnosticReadFacts,
}
impl DiagnosticSummaryMigrationArchiveCommit {
    /// Observe the actual current operation that settled this migration.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.current
    }
    /// Observe the complete preserved historical Journal identity.
    pub const fn historical_operation(&self) -> LogicalStoreOperationKey {
        self.historical
    }
    /// Observe actual Archive facts after checked closure.
    pub fn facts(&self) -> DiagnosticReadFacts {
        self.facts
    }
}
/// Observe validated closed original-copy preservation; grants no Summary mutation.
pub struct DiagnosticSummaryMigrationArchiveObservation {
    pub(super) current: LogicalStoreOperationKey,
    pub(super) historical: LogicalStoreOperationKey,
    pub(super) original: DiagnosticReadFacts,
}
impl DiagnosticSummaryMigrationArchiveObservation {
    /// Observe the current operation that checked actual archive/copy closure.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.current
    }
    /// Observe full historical provenance without granting replay authority.
    pub const fn historical_operation(&self) -> LogicalStoreOperationKey {
        self.historical
    }
    /// Observe the preserved copy's actual identity, digest and byte length.
    pub fn original_stage(&self) -> DiagnosticReadFacts {
        self.original
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::diagnostic_storage) enum MigrationRole {
    Original,
    Candidate,
    Journal,
    Archive,
}
pub(in crate::diagnostic_storage) struct MigrationIntent {
    pub(super) id: u64,
    pub(super) object: DiagnosticObject,
    pub(super) original_read: Option<u64>,
    pub(super) original_stage: Option<u64>,
    pub(super) candidate_stage: Option<u64>,
    pub(super) journal_stage: Option<u64>,
    pub(super) journal_read: Option<u64>,
    pub(super) journal_facts: Option<DiagnosticReadFacts>,
    pub(super) journal_attempted: bool,
    pub(super) archive_stage: Option<u64>,
    pub(super) archive_sha256: Option<[u8; 32]>,
    pub(super) archive_bytes: Option<Vec<u8>>,
    pub(super) plan: Option<DiagnosticSummaryMigrationPlan>,
    pub(super) case: DiagnosticSummaryMigrationCase,
    pub(super) validating: bool,
    pub(super) semantic_verified: bool,
    pub(super) candidate_active: Option<DiagnosticReadFacts>,
    pub(super) archive_active: Option<DiagnosticReadFacts>,
}
pub(in crate::diagnostic_storage) struct MigrationArchive {
    pub(super) historical: LogicalStoreOperationKey,
    pub(super) object: DiagnosticObject,
    pub(super) archive: DiagnosticReadFacts,
    pub(super) original: DiagnosticReadFacts,
    pub(super) generation: u64,
}
