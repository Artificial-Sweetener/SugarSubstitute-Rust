// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve capture disposition matching data independently of live native authority.
use super::{
    IncidentStoreError,
    codec_evidence::DiscardRecord,
    codec_identity::{OperationRecord, OwnerRecord, SubjectRecord},
};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::{
    incident::{
        BuildIdentity, DumpId, IncidentKey, IncidentRevision, LogicalStoreOperationKey,
        MAX_PAYLOAD_BYTES, PayloadDiscard,
    },
    lifecycle::CoordinatorKey,
};

/// Complete recorded expectations; these values grant no file or native observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureRecordedFileFacts {
    identity: [u8; 24],
    sha256: [u8; 32],
    bytes: u64,
}
impl CaptureRecordedFileFacts {
    /// Validate nonempty recorded file data within the hard 256-MiB limit.
    /// # Errors
    /// Rejects zero identity/digest/bytes and excessive bytes. Consumers apply stricter bounds.
    pub fn new(
        identity: [u8; 24],
        sha256: [u8; 32],
        bytes: u64,
    ) -> Result<Self, IncidentStoreError> {
        if identity == [0; 24] || sha256 == [0; 32] || bytes == 0 || bytes > MAX_PAYLOAD_BYTES {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(Self { identity, sha256, bytes })
    }
    /// Return matching file identity without an actual handle.
    pub const fn identity(&self) -> [u8; 24] {
        self.identity
    }
    /// Return the recorded complete-content SHA-256.
    pub const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    /// Return recorded nonempty bytes.
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}
/// Closed historical metadata stage purpose, never an activation capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMetadataStageRole {
    /// Exact copy of the original metadata image.
    OriginalMetadata,
    /// Exact candidate image with the selected 64-byte record removed.
    CandidateMetadata,
}
/// Recorded exact committed completion; actual protected leases verify these expectations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureDispositionCompletion {
    /// Exact summary and immutable Minidump payload for the plan's incident/revision.
    Promoted {
        /// Recorded checked summary file.
        summary_file: CaptureRecordedFileFacts,
        /// Recorded payload whose whole digest and bytes equal the source.
        payload_file: CaptureRecordedFileFacts,
    },
    /// Exact summary durably describes the source's explicit discard.
    Discarded {
        /// Recorded checked summary file.
        summary_file: CaptureRecordedFileFacts,
        /// Explicit categorical disposition, independently validated against the summary.
        reason: PayloadDiscard,
        /// Exact nonempty source size.
        source_bytes: u64,
    },
}
/// Strict inert historical capture intent; decoding never mints a disposition permit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureDispositionPlan {
    historical_operation: LogicalStoreOperationKey,
    incident: IncidentKey,
    incident_revision: IncidentRevision,
    capture_generation: CoordinatorKey,
    uuid: DumpId,
    build: BuildIdentity,
    database_identity: [u8; 24],
    metrics_identity: [u8; 24],
    source: CaptureRecordedFileFacts,
    metadata_preimage: CaptureRecordedFileFacts,
    original_stage: CaptureRecordedFileFacts,
    candidate_stage: CaptureRecordedFileFacts,
    completion: CaptureDispositionCompletion,
}
impl CaptureDispositionPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        historical_operation: LogicalStoreOperationKey,
        incident: IncidentKey,
        incident_revision: IncidentRevision,
        capture_generation: CoordinatorKey,
        uuid: DumpId,
        build: BuildIdentity,
        database_identity: [u8; 24],
        metrics_identity: [u8; 24],
        source: CaptureRecordedFileFacts,
        metadata_preimage: CaptureRecordedFileFacts,
        original_stage: CaptureRecordedFileFacts,
        candidate_stage: CaptureRecordedFileFacts,
        completion: CaptureDispositionCompletion,
    ) -> Result<Self, IncidentStoreError> {
        let summary = match completion {
            CaptureDispositionCompletion::Promoted { summary_file, payload_file } => {
                if payload_file.bytes != source.bytes || payload_file.sha256 != source.sha256 {
                    return Err(IncidentStoreError::RecoveryConflict);
                }
                summary_file
            }
            CaptureDispositionCompletion::Discarded { summary_file, source_bytes, .. } => {
                if source_bytes != source.bytes {
                    return Err(IncidentStoreError::RecoveryConflict);
                }
                summary_file
            }
        };
        if historical_operation.owner().namespace != incident.subject().coordinator().namespace
            || capture_generation != incident.subject().coordinator()
            || database_identity == [0; 24]
            || metrics_identity == [0; 24]
            || metadata_preimage.bytes > 1_048_576
            || original_stage.bytes > 1_048_576
            || candidate_stage.bytes > 1_048_576
            || summary.bytes > 65_536
            || original_stage.bytes != metadata_preimage.bytes
            || original_stage.sha256 != metadata_preimage.sha256
            || metadata_preimage.bytes.checked_sub(64) != Some(candidate_stage.bytes)
            || original_stage.sha256 == candidate_stage.sha256
            || original_stage.identity == candidate_stage.identity
            || [original_stage.identity, candidate_stage.identity].iter().any(|identity| {
                *identity == metadata_preimage.identity || *identity == source.identity
            })
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(Self {
            historical_operation,
            incident,
            incident_revision,
            capture_generation,
            uuid,
            build,
            database_identity,
            metrics_identity,
            source,
            metadata_preimage,
            original_stage,
            candidate_stage,
            completion,
        })
    }
    /// Return the exact historical full operation, never the current recovery authority.
    pub const fn historical_operation(&self) -> LogicalStoreOperationKey {
        self.historical_operation
    }
    /// Return the full incident subject.
    pub const fn incident(&self) -> IncidentKey {
        self.incident
    }
    /// Return the exact committed completion revision.
    pub const fn incident_revision(&self) -> IncidentRevision {
        self.incident_revision
    }
    /// Return the historical actual-generation matching key.
    pub const fn capture_generation(&self) -> CoordinatorKey {
        self.capture_generation
    }
    /// Return the nonzero canonical dump UUID matching bytes.
    pub const fn uuid(&self) -> DumpId {
        self.uuid
    }
    /// Borrow the checked historical build annotation.
    pub const fn build(&self) -> &BuildIdentity {
        &self.build
    }
    /// Return recorded database directory identity.
    pub const fn database_identity(&self) -> [u8; 24] {
        self.database_identity
    }
    /// Return recorded metrics file identity.
    pub const fn metrics_identity(&self) -> [u8; 24] {
        self.metrics_identity
    }
    /// Return recorded source identity/hash/bytes without a read lease.
    pub const fn source(&self) -> CaptureRecordedFileFacts {
        self.source
    }
    /// Return the original complete metadata image expectations.
    pub const fn metadata_preimage(&self) -> CaptureRecordedFileFacts {
        self.metadata_preimage
    }
    /// Return recorded actual original-stage file expectations.
    pub const fn original_stage(&self) -> CaptureRecordedFileFacts {
        self.original_stage
    }
    /// Return recorded actual candidate-stage file expectations.
    pub const fn candidate_stage(&self) -> CaptureRecordedFileFacts {
        self.candidate_stage
    }
    /// Return matching completion data without authenticating its durability.
    pub const fn completion(&self) -> CaptureDispositionCompletion {
        self.completion
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRecord {
    identity: [u8; 24],
    sha256: [u8; 32],
    bytes: u64,
}
impl FileRecord {
    fn into_facts(self) -> Result<CaptureRecordedFileFacts, IncidentStoreError> {
        CaptureRecordedFileFacts::new(self.identity, self.sha256, self.bytes)
    }
}
impl From<CaptureRecordedFileFacts> for FileRecord {
    fn from(file: CaptureRecordedFileFacts) -> Self {
        Self { identity: file.identity, sha256: file.sha256, bytes: file.bytes }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageRecord {
    role: CaptureMetadataStageRole,
    identity: [u8; 24],
    sha256: [u8; 32],
    bytes: u64,
}
impl StageRecord {
    fn into_facts(
        self,
        expected: CaptureMetadataStageRole,
    ) -> Result<CaptureRecordedFileFacts, IncidentStoreError> {
        if self.role != expected {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        CaptureRecordedFileFacts::new(self.identity, self.sha256, self.bytes)
    }
    fn from_facts(role: CaptureMetadataStageRole, file: CaptureRecordedFileFacts) -> Self {
        Self { role, identity: file.identity, sha256: file.sha256, bytes: file.bytes }
    }
}
mod completion;
use completion::CompletionRecord;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaptureJournalRecord {
    historical_operation: OperationRecord,
    incident: SubjectRecord,
    incident_revision: u64,
    capture_generation: OwnerRecord,
    uuid: [u8; 16],
    build: super::codec_text::BuildRecord,
    database_identity: [u8; 24],
    metrics_identity: [u8; 24],
    source: FileRecord,
    metadata_preimage: FileRecord,
    original_stage: StageRecord,
    candidate_stage: StageRecord,
    completion: CompletionRecord,
}
impl CaptureJournalRecord {
    pub(super) fn into_plan(self) -> Result<CaptureDispositionPlan, IncidentStoreError> {
        let completion = match self.completion {
            CompletionRecord::Promoted { summary_file, payload_file } => {
                CaptureDispositionCompletion::Promoted {
                    summary_file: summary_file.into_facts()?,
                    payload_file: payload_file.into_facts()?,
                }
            }
            CompletionRecord::Discarded { summary_file, reason, source_bytes } => {
                CaptureDispositionCompletion::Discarded {
                    summary_file: summary_file.into_facts()?,
                    reason: reason.into_facts(),
                    source_bytes,
                }
            }
        };
        CaptureDispositionPlan::new(
            self.historical_operation.into_operation()?,
            self.incident.into_key()?,
            IncidentRevision::new(self.incident_revision)?,
            self.capture_generation.into_owner()?,
            DumpId::new(self.uuid)?,
            self.build.into_facts()?,
            self.database_identity,
            self.metrics_identity,
            self.source.into_facts()?,
            self.metadata_preimage.into_facts()?,
            self.original_stage.into_facts(CaptureMetadataStageRole::OriginalMetadata)?,
            self.candidate_stage.into_facts(CaptureMetadataStageRole::CandidateMetadata)?,
            completion,
        )
    }
}
impl From<&CaptureDispositionPlan> for CaptureJournalRecord {
    fn from(plan: &CaptureDispositionPlan) -> Self {
        let completion = match plan.completion {
            CaptureDispositionCompletion::Promoted { summary_file, payload_file } => {
                CompletionRecord::Promoted {
                    summary_file: summary_file.into(),
                    payload_file: payload_file.into(),
                }
            }
            CaptureDispositionCompletion::Discarded { summary_file, reason, source_bytes } => {
                CompletionRecord::Discarded {
                    summary_file: summary_file.into(),
                    reason: reason.into(),
                    source_bytes,
                }
            }
        };
        Self {
            historical_operation: plan.historical_operation.into(),
            incident: plan.incident.into(),
            incident_revision: plan.incident_revision.get(),
            capture_generation: plan.capture_generation.into(),
            uuid: *plan.uuid.bytes(),
            build: (&plan.build).into(),
            database_identity: plan.database_identity,
            metrics_identity: plan.metrics_identity,
            source: plan.source.into(),
            metadata_preimage: plan.metadata_preimage.into(),
            original_stage: StageRecord::from_facts(
                CaptureMetadataStageRole::OriginalMetadata,
                plan.original_stage,
            ),
            candidate_stage: StageRecord::from_facts(
                CaptureMetadataStageRole::CandidateMetadata,
                plan.candidate_stage,
            ),
            completion,
        }
    }
}
