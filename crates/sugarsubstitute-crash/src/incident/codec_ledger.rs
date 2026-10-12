// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate an exact eviction tombstone and its single checked ledger transition before effects.
use super::{
    IncidentStoreError,
    codec::{STATE_BOUND, decode, encode, required_option, utc},
    codec_capture::{CaptureDispositionPlan, CaptureJournalRecord},
    codec_identity::{OperationRecord, SubjectRecord},
    codec_migration::{
        MigrationArchiveRecord, MigrationRecord, SummaryMigrationArchive, SummaryMigrationJournal,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{
    incident::{
        IncidentKey, IncidentRevision, LogicalStoreOperationKey, SummaryEviction,
        SummaryEvictionKind, SummaryEvictions,
    },
    lifecycle::NamespaceId,
};

/// Recorded expected file facts; decoding these bytes creates no native observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidentFileFacts {
    identity: [u8; 24],
    sha256: [u8; 32],
    bytes: u64,
}
impl IncidentFileFacts {
    /// Validate a nonempty metadata object within its format bound.
    /// # Errors
    /// Rejects zero identity/digest, zero bytes or more than 64 KiB.
    pub fn new(
        identity: [u8; 24],
        sha256: [u8; 32],
        bytes: u64,
    ) -> Result<Self, IncidentStoreError> {
        if identity == [0; 24] || sha256 == [0; 32] || bytes == 0 || bytes > STATE_BOUND as u64 {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(Self { identity, sha256, bytes })
    }
    /// Return recorded expected file identity, never an actual open handle.
    pub const fn identity(self) -> [u8; 24] {
        self.identity
    }
    /// Return recorded expected complete-content digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
    /// Return recorded expected content bytes.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

/// Exact durable intent data for deleting one summary and applying one ledger increment.
/// Its constructor validates arithmetic and canonical bytes, without deleting or settling anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryEvictionJournal {
    operation: LogicalStoreOperationKey,
    revision: IncidentRevision,
    victim: IncidentFileFacts,
    original_file: IncidentFileFacts,
    candidate_stage: IncidentFileFacts,
    original: SummaryEvictions,
    candidate: SummaryEvictions,
    candidate_sha256: [u8; 32],
}
impl SummaryEvictionJournal {
    /// Bind exact expected victim and original ledger to one validated candidate stage.
    /// # Errors
    /// Rejects foreign namespaces, invalid stage identity, changed canonical digest/size,
    /// or a candidate other than exactly one checked increment of the original.
    pub fn new(
        operation: LogicalStoreOperationKey,
        revision: IncidentRevision,
        victim: IncidentFileFacts,
        original_file: IncidentFileFacts,
        candidate_stage: IncidentFileFacts,
        original: SummaryEvictions,
        candidate: SummaryEvictions,
    ) -> Result<Self, IncidentStoreError> {
        let eviction = candidate.last().ok_or(IncidentStoreError::RecoveryConflict)?;
        let original_bytes = encode_store_state(&IncidentStoreState::Ledger(original))?;
        if candidate_stage.identity == original_file.identity
            || operation.owner().namespace != original.namespace()
            || candidate.namespace() != original.namespace()
            || original.record(eviction)? != candidate
            || original_file.sha256 != digest(&original_bytes)
            || original_file.bytes != original_bytes.len() as u64
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let candidate_bytes = encode_store_state(&IncidentStoreState::Ledger(candidate))?;
        let candidate_sha256 = digest(&candidate_bytes);
        if candidate_stage.sha256 != candidate_sha256
            || candidate_stage.bytes != candidate_bytes.len() as u64
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(Self {
            operation,
            revision,
            victim,
            original_file,
            candidate_stage,
            original,
            candidate,
            candidate_sha256,
        })
    }
    /// Return the historical full coordinator/operation identity without renewing it.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Return exact selected summary identity from the candidate's last eviction.
    pub fn incident(&self) -> Result<IncidentKey, IncidentStoreError> {
        self.candidate
            .last()
            .map(SummaryEviction::incident)
            .ok_or(IncidentStoreError::RecoveryConflict)
    }
    /// Return the selected summary revision.
    pub const fn revision(&self) -> IncidentRevision {
        self.revision
    }
    /// Return recorded victim facts, distinct from an actual read observation.
    pub const fn victim(&self) -> IncidentFileFacts {
        self.victim
    }
    /// Return recorded original ledger identity/digest/bytes.
    pub const fn original_file(&self) -> IncidentFileFacts {
        self.original_file
    }
    /// Return complete recorded candidate stage facts, never a recreated inode.
    pub const fn candidate_stage(&self) -> IncidentFileFacts {
        self.candidate_stage
    }
    /// Return the effective ledger before actual deletion.
    pub const fn original(&self) -> SummaryEvictions {
        self.original
    }
    /// Return the candidate applied only after checked deletion.
    pub const fn candidate(&self) -> SummaryEvictions {
        self.candidate
    }
    /// Return the recomputed exact canonical candidate digest.
    pub const fn candidate_sha256(&self) -> [u8; 32] {
        self.candidate_sha256
    }
}
/// Closed schema-one store records; journals share the registered store-state format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncidentStoreState {
    /// Current immutable cumulative ledger.
    Ledger(SummaryEvictions),
    /// One victim-bound summary-eviction tombstone awaiting actual settlement.
    SummaryEviction(Box<SummaryEvictionJournal>),
    /// Exact historical capture stages and committed completion, without native authority.
    CapturePromotion(Box<CaptureDispositionPlan>),
    /// Exact original-copy and candidate expectations awaiting checked migration.
    SummaryMigration(Box<SummaryMigrationJournal>),
    /// Recorded original-copy preservation after migration; never mutation authority.
    SummaryMigrationArchive(Box<SummaryMigrationArchive>),
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case", deny_unknown_fields)]
enum StoreRecord {
    Ledger { ledger: LedgerRecord },
    SummaryEviction { journal: Box<JournalRecord> },
    CapturePromotion { journal: Box<CaptureJournalRecord> },
    SummaryMigration(Box<MigrationRecord>),
    SummaryMigrationArchive(Box<MigrationArchiveRecord>),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerRecord {
    namespace: [u8; 32],
    pending: u64,
    acknowledged: u64,
    quarantined: u64,
    #[serde(deserialize_with = "required_option")]
    last: Option<EvictionRecord>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvictionRecord {
    incident: SubjectRecord,
    at: u64,
    category: CategoryRecord,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CategoryRecord {
    Pending,
    Acknowledged,
    Quarantined,
}
impl LedgerRecord {
    fn into_ledger(self) -> Result<SummaryEvictions, IncidentStoreError> {
        SummaryEvictions::new(
            NamespaceId::new(self.namespace),
            self.pending,
            self.acknowledged,
            self.quarantined,
            self.last
                .map(|last| {
                    Ok::<_, IncidentStoreError>(SummaryEviction::new(
                        last.incident.into_key()?,
                        utc(last.at)?,
                        match last.category {
                            CategoryRecord::Pending => SummaryEvictionKind::Pending,
                            CategoryRecord::Acknowledged => SummaryEvictionKind::Acknowledged,
                            CategoryRecord::Quarantined => SummaryEvictionKind::Quarantined,
                        },
                    ))
                })
                .transpose()?,
        )
        .map_err(Into::into)
    }
}
impl From<SummaryEvictions> for LedgerRecord {
    fn from(value: SummaryEvictions) -> Self {
        Self {
            namespace: *value.namespace().bytes(),
            pending: value.pending(),
            acknowledged: value.acknowledged(),
            quarantined: value.quarantined(),
            last: value.last().map(|last| EvictionRecord {
                incident: last.incident().into(),
                at: last.at().unix_seconds(),
                category: match last.kind() {
                    SummaryEvictionKind::Pending => CategoryRecord::Pending,
                    SummaryEvictionKind::Acknowledged => CategoryRecord::Acknowledged,
                    SummaryEvictionKind::Quarantined => CategoryRecord::Quarantined,
                },
            }),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileRecord {
    identity: [u8; 24],
    sha256: [u8; 32],
    bytes: u64,
}
impl FileRecord {
    pub(super) fn into_facts(self) -> Result<IncidentFileFacts, IncidentStoreError> {
        IncidentFileFacts::new(self.identity, self.sha256, self.bytes)
    }
}
impl From<IncidentFileFacts> for FileRecord {
    fn from(value: IncidentFileFacts) -> Self {
        Self { identity: value.identity, sha256: value.sha256, bytes: value.bytes }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalRecord {
    operation: OperationRecord,
    revision: u64,
    victim: FileRecord,
    original_file: FileRecord,
    candidate_stage: FileRecord,
    original: LedgerRecord,
    original_sha256: [u8; 32],
    candidate: LedgerRecord,
    candidate_sha256: [u8; 32],
}

/// Encode canonical ledger or journal bytes within the shared 64-KiB format bound.
/// # Errors
/// Returns strict encoding/bound errors; construction remains separate from durability.
pub fn encode_store_state(state: &IncidentStoreState) -> Result<Vec<u8>, IncidentStoreError> {
    encode(
        match state {
            IncidentStoreState::Ledger(ledger) => StoreRecord::Ledger { ledger: (*ledger).into() },
            IncidentStoreState::SummaryEviction(journal) => StoreRecord::SummaryEviction {
                journal: Box::new(JournalRecord {
                    operation: journal.operation.into(),
                    revision: journal.revision.get(),
                    victim: journal.victim.into(),
                    original_file: journal.original_file.into(),
                    candidate_stage: journal.candidate_stage.into(),
                    original: journal.original.into(),
                    original_sha256: journal.original_file.sha256,
                    candidate: journal.candidate.into(),
                    candidate_sha256: journal.candidate_sha256,
                }),
            },
            IncidentStoreState::SummaryMigration(journal) => {
                StoreRecord::SummaryMigration(Box::new(journal.as_ref().into()))
            }
            IncidentStoreState::SummaryMigrationArchive(archive) => {
                StoreRecord::SummaryMigrationArchive(Box::new(archive.as_ref().into()))
            }
            IncidentStoreState::CapturePromotion(plan) => {
                StoreRecord::CapturePromotion { journal: Box::new(plan.as_ref().into()) }
            }
        },
        STATE_BOUND,
    )
}
/// Decode strict schema-one store bytes and revalidate both ledger hashes and arithmetic.
/// # Errors
/// Rejects unknown variants/fields/versions, third states, overflow and identity mismatch.
pub fn decode_store_state(bytes: &[u8]) -> Result<IncidentStoreState, IncidentStoreError> {
    Ok(match decode::<StoreRecord>(bytes, STATE_BOUND)? {
        StoreRecord::Ledger { ledger } => IncidentStoreState::Ledger(ledger.into_ledger()?),
        StoreRecord::SummaryMigration(journal) => {
            IncidentStoreState::SummaryMigration(Box::new(journal.into_journal()?))
        }
        StoreRecord::SummaryMigrationArchive(archive) => {
            IncidentStoreState::SummaryMigrationArchive(Box::new(archive.into_archive()?))
        }
        StoreRecord::CapturePromotion { journal } => {
            IncidentStoreState::CapturePromotion(Box::new(journal.into_plan()?))
        }
        StoreRecord::SummaryEviction { journal } => {
            let expected_original = journal.original_sha256;
            let expected_candidate = journal.candidate_sha256;
            let journal = SummaryEvictionJournal::new(
                journal.operation.into_operation()?,
                IncidentRevision::new(journal.revision)?,
                journal.victim.into_facts()?,
                journal.original_file.into_facts()?,
                journal.candidate_stage.into_facts()?,
                journal.original.into_ledger()?,
                journal.candidate.into_ledger()?,
            )?;
            if journal.original_file.sha256 != expected_original
                || journal.candidate_sha256 != expected_candidate
            {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            IncidentStoreState::SummaryEviction(Box::new(journal))
        }
    })
}
pub(super) fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
