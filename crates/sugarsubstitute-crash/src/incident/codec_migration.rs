// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact legacy summary copies and closed recorded migration facts without authority.
use super::{
    IncidentFileFacts, IncidentStoreError,
    codec::required_option,
    codec_identity::{OperationRecord, SubjectRecord},
    codec_ledger::FileRecord,
};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::incident::{
    IncidentKey, IncidentRevision, LogicalStoreOperationKey,
};

/// Recorded exact summary migration intent; construction grants no native authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryMigrationJournal {
    operation: LogicalStoreOperationKey,
    incident: IncidentKey,
    revision: IncidentRevision,
    payload_revision: Option<IncidentRevision>,
    original_file: IncidentFileFacts,
    original_stage: IncidentFileFacts,
    candidate_stage: IncidentFileFacts,
}
impl SummaryMigrationJournal {
    /// Bind one historical operation and its distinct exact original/candidate inodes.
    /// Schemas are fixed one to two; actual conversion and native custody are separate.
    /// # Errors
    /// Rejects foreign namespaces, changed original-copy bytes, repeated identities,
    /// equal original/candidate digests or a payload generation beyond the summary.
    pub fn new(
        operation: LogicalStoreOperationKey,
        incident: IncidentKey,
        revision: IncidentRevision,
        payload_revision: Option<IncidentRevision>,
        original_file: IncidentFileFacts,
        original_stage: IncidentFileFacts,
        candidate_stage: IncidentFileFacts,
    ) -> Result<Self, IncidentStoreError> {
        if operation.owner().namespace != incident.subject().coordinator().namespace
            || original_file.identity() == original_stage.identity()
            || original_file.identity() == candidate_stage.identity()
            || original_stage.identity() == candidate_stage.identity()
            || original_file.sha256() != original_stage.sha256()
            || original_file.bytes() != original_stage.bytes()
            || original_file.sha256() == candidate_stage.sha256()
            || payload_revision.is_some_and(|payload| payload > revision)
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(Self {
            operation,
            incident,
            revision,
            payload_revision,
            original_file,
            original_stage,
            candidate_stage,
        })
    }
    /// Observe the complete historical coordinator and operation key.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Observe the exact subject whose summary bytes were migrated.
    pub const fn incident(&self) -> IncidentKey {
        self.incident
    }
    /// Observe the summary revision unchanged by format conversion.
    pub const fn revision(&self) -> IncidentRevision {
        self.revision
    }
    /// Observe immutable payload generation independently of later summary revisions.
    pub const fn payload_revision(&self) -> Option<IncidentRevision> {
        self.payload_revision
    }
    /// Observe recorded original active summary expectations, never an open inode.
    pub const fn original_file(&self) -> IncidentFileFacts {
        self.original_file
    }
    /// Observe the exact preserved original-copy stage expectations.
    pub const fn original_stage(&self) -> IncidentFileFacts {
        self.original_stage
    }
    /// Observe the exact candidate inode which becomes the active summary.
    pub const fn candidate_stage(&self) -> IncidentFileFacts {
        self.candidate_stage
    }
}
/// Inert preservation record for one original copy after migration settlement.
/// It remains valid when the current summary advances or is evicted; it grants no CAS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryMigrationArchive {
    migration: SummaryMigrationJournal,
}
impl SummaryMigrationArchive {
    /// Record validated migration matching data, independently of actual settlement.
    /// # Errors
    /// Returns the journal's identity/namespace/fact rejection if expectations conflict.
    pub fn new(journal: SummaryMigrationJournal) -> Result<Self, IncidentStoreError> {
        let journal = SummaryMigrationJournal::new(
            journal.operation,
            journal.incident,
            journal.revision,
            journal.payload_revision,
            journal.original_file,
            journal.original_stage,
            journal.candidate_stage,
        )?;
        Ok(Self { migration: journal })
    }
    /// Borrow exact historical preservation data without native permission.
    pub const fn migration(&self) -> &SummaryMigrationJournal {
        &self.migration
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MigrationRecord {
    operation: OperationRecord,
    incident: SubjectRecord,
    revision: u64,
    #[serde(deserialize_with = "required_option")]
    payload_revision: Option<u64>,
    source_schema: u32,
    candidate_schema: u32,
    original_file: FileRecord,
    original_stage: FileRecord,
    candidate_stage: FileRecord,
}
impl From<&SummaryMigrationJournal> for MigrationRecord {
    fn from(journal: &SummaryMigrationJournal) -> Self {
        Self {
            operation: journal.operation.into(),
            incident: journal.incident.into(),
            revision: journal.revision.get(),
            payload_revision: journal.payload_revision.map(IncidentRevision::get),
            source_schema: 1,
            candidate_schema: 2,
            original_file: journal.original_file.into(),
            original_stage: journal.original_stage.into(),
            candidate_stage: journal.candidate_stage.into(),
        }
    }
}
impl MigrationRecord {
    pub(super) fn into_journal(self) -> Result<SummaryMigrationJournal, IncidentStoreError> {
        if self.source_schema != 1 || self.candidate_schema != 2 {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        SummaryMigrationJournal::new(
            self.operation.into_operation()?,
            self.incident.into_key()?,
            IncidentRevision::new(self.revision)?,
            self.payload_revision.map(IncidentRevision::new).transpose()?,
            self.original_file.into_facts()?,
            self.original_stage.into_facts()?,
            self.candidate_stage.into_facts()?,
        )
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MigrationArchiveRecord {
    migration: MigrationRecord,
}
impl From<&SummaryMigrationArchive> for MigrationArchiveRecord {
    fn from(archive: &SummaryMigrationArchive) -> Self {
        Self { migration: archive.migration().into() }
    }
}
impl MigrationArchiveRecord {
    pub(super) fn into_archive(self) -> Result<SummaryMigrationArchive, IncidentStoreError> {
        SummaryMigrationArchive::new(self.migration.into_journal()?)
    }
}
