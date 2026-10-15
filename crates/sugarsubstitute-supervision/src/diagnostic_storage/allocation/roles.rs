// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive closed allocation destinations from the authoritative owned types.
use super::*;
use std::{mem::size_of, sync::atomic::AtomicUsize};
use sugarsubstitute_contracts::incident::{
    AttachmentFacts, AttachmentKind, BuildIdentity, FileLabel, IncidentKey, IncidentRevision,
    IncidentSeed, IncidentSummary, IncidentUtc, MAX_ATTACHMENTS, MAX_SUMMARIES, SummaryEvictions,
    SymbolName,
};

#[repr(C)]
struct ArcAllocation<T> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: T,
}
pub(super) const LEDGER_CONTROL_BYTES: usize = size_of::<ArcAllocation<ledger::Ledger>>();
pub(super) const CORRELATION_CONTROL_BYTES: usize = size_of::<ArcAllocation<()>>();
pub(super) const OWNER_BASE_BYTES: usize = size_of::<ArcAllocation<Mutex<State>>>()
    + DIAGNOSTIC_STAGE_BOUND * size_of::<stage::RetainedStage>()
    + DIAGNOSTIC_READ_BOUND * size_of::<read::RetainedRead>()
    + DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND * size_of::<summary_migration::MigrationArchive>();
#[repr(C)]
struct SummaryResult {
    summary: IncidentSummary,
    payload_revision: Option<IncidentRevision>,
    operation: LogicalStoreOperationKey,
    activation: DiagnosticActivation,
    ledger: SummaryEvictions,
    acknowledged: Option<IncidentUtc>,
    allocation: DiagnosticRecoveryRecordLoan,
}
#[repr(C)]
struct SeedResult {
    seed: IncidentSeed,
    operation: LogicalStoreOperationKey,
    activation: DiagnosticActivation,
    allocation: DiagnosticRecoverySeedLoan,
}
#[repr(C)]
struct SummaryCell {
    summary: IncidentSummary,
    payload_revision: Option<IncidentRevision>,
    facts: DiagnosticReadFacts,
}
#[repr(C)]
struct SummaryCollection {
    keys: Vec<IncidentKey>,
    records: Vec<SummaryCell>,
    allocation: DiagnosticRecoveryCollectionLoan,
}
#[repr(C)]
struct PublicationPayload {
    kind: AttachmentKind,
    retained: u64,
    original: u64,
    omitted: u64,
    actual: DiagnosticActivation,
}
type ExpectedSummary = Option<(IncidentRevision, [u8; 32])>;
#[repr(C)]
struct PublicationDemand {
    payload_bytes: u64,
    payload_groups: usize,
    summary_slots: usize,
    staging_headroom: u64,
}
#[repr(C)]
struct RetainedPreflight {
    id: u64,
    seed: IncidentSeed,
    revision: IncidentRevision,
    expected: ExpectedSummary,
    native: DiagnosticPredecessor,
    demand: PublicationDemand,
    context: JobContext,
    allocation: DiagnosticRetainedPreflightLoan,
}
#[repr(C)]
struct PendingPublication {
    id: u64,
    seed: IncidentSeed,
    revision: IncidentRevision,
    expected: DiagnosticPredecessor,
    payloads: Vec<PublicationPayload>,
    demand: PublicationDemand,
    context: JobContext,
    allocation: DiagnosticPendingPublicationLoan,
}
#[repr(C)]
struct PublicationToken {
    id: u64,
    operation: LogicalStoreOperationKey,
    identity: DiagnosticPublicationIdentity,
}
const SUMMARY_DYNAMIC: usize = 2 * BuildIdentity::MAX_BYTES
    + 2 * FileLabel::MAX_BYTES
    + SymbolName::MAX_BYTES
    + size_of::<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>();
/// Inline upper envelope of each escaped Summary/reporting result, in bytes.
pub const DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES: usize = size_of::<SummaryResult>();
/// Inline upper envelope of each escaped seed result, in bytes.
pub const DIAGNOSTIC_SEED_RESULT_INLINE_BYTES: usize = size_of::<SeedResult>();
/// Inline upper envelope of an owned Summary/facts collection cell, in bytes.
pub const DIAGNOSTIC_SUMMARY_COLLECTION_CELL_BYTES: usize = size_of::<SummaryCell>();
/// Inline upper envelope of both owned collection vectors and custody, in bytes.
pub const DIAGNOSTIC_SUMMARY_COLLECTION_INLINE_BYTES: usize = size_of::<SummaryCollection>();
/// Inline upper envelope of an actual pending publication and custody, in bytes.
pub const DIAGNOSTIC_PENDING_PUBLICATION_INLINE_BYTES: usize = size_of::<PendingPublication>();
/// Inline upper envelope of each pending attachment tuple, in bytes.
pub const DIAGNOSTIC_PUBLICATION_PAYLOAD_INLINE_BYTES: usize = size_of::<PublicationPayload>();
/// Inline upper envelope of the retained preflight/context destination, in bytes.
pub const DIAGNOSTIC_RETAINED_PREFLIGHT_INLINE_BYTES: usize = size_of::<RetainedPreflight>();
/// Inline upper envelope of the complete current-summary expectation, in bytes.
pub const DIAGNOSTIC_EXPECTED_SUMMARY_INLINE_BYTES: usize = size_of::<ExpectedSummary>();
/// Inline upper envelope of all four publication demand fields, in bytes.
pub const DIAGNOSTIC_PUBLICATION_DEMAND_INLINE_BYTES: usize = size_of::<PublicationDemand>();
/// Inline upper envelope of either escaped publication token, in bytes.
pub const DIAGNOSTIC_PUBLICATION_TOKEN_INLINE_BYTES: usize = size_of::<PublicationToken>();
const COLLECTION_ROLE: usize = DIAGNOSTIC_SUMMARY_COLLECTION_INLINE_BYTES
    + MAX_SUMMARIES
        * (size_of::<IncidentKey>() + DIAGNOSTIC_SUMMARY_COLLECTION_CELL_BYTES + SUMMARY_DYNAMIC);
const _: () = assert!(COLLECTION_ROLE <= DIAGNOSTIC_COLLECTION_BYTES);
const _: () = assert!(
    DIAGNOSTIC_COLLECTION_BYTES + DIAGNOSTIC_DECODE_BYTES + DIAGNOSTIC_RESIDUAL_BYTES
        == DIAGNOSTIC_STORAGE_BYTES as usize
);

pub(super) enum ResidualRole {
    Record,
    Seed,
    Preflight,
    Pending,
    Correlation,
    PublicationToken,
    Buffer,
    WholeSource,
    Inventory(usize),
    ScanBinding(usize),
    PayloadDecision,
    PayloadPreparation,
    PayloadIntent,
    MigrationIntent,
    IncidentValidationInner,
    IncidentValidationOuter,
}
impl ResidualRole {
    pub(super) const fn bytes(&self) -> usize {
        match self {
            Self::IncidentValidationInner => incident_validation::inner_destination_bytes(),
            Self::IncidentValidationOuter => incident_validation::outer_destination_bytes(),
            Self::Record => DIAGNOSTIC_SUMMARY_RESULT_INLINE_BYTES + SUMMARY_DYNAMIC,
            Self::Seed => DIAGNOSTIC_SEED_RESULT_INLINE_BYTES + BuildIdentity::MAX_BYTES,
            Self::Preflight => {
                DIAGNOSTIC_RETAINED_PREFLIGHT_INLINE_BYTES + BuildIdentity::MAX_BYTES
            }
            Self::Pending => {
                DIAGNOSTIC_PENDING_PUBLICATION_INLINE_BYTES
                    + MAX_ATTACHMENTS * DIAGNOSTIC_PUBLICATION_PAYLOAD_INLINE_BYTES
                    + BuildIdentity::MAX_BYTES
            }
            Self::Correlation => CORRELATION_CONTROL_BYTES,
            Self::PublicationToken => DIAGNOSTIC_PUBLICATION_TOKEN_INLINE_BYTES,
            Self::Buffer => size_of::<DiagnosticRecordBuffer>() + DIAGNOSTIC_IO_BYTES,
            Self::WholeSource => size_of::<DiagnosticRecoverySource>() + DIAGNOSTIC_IO_BYTES,
            Self::Inventory(entries) => {
                size_of::<DiagnosticInventory>() + *entries * size_of::<DiagnosticInventoryEntry>()
            }
            Self::ScanBinding(entries) => {
                // Digest sets retain full capacity for later admitted stage mutations.
                *entries * size_of::<inventory::ObservedEntry>()
                    + DIAGNOSTIC_SCAN_BOUND * 3 * size_of::<[u8; 32]>()
            }
            Self::PayloadDecision => {
                size_of::<DiagnosticPayloadDiscardDecision>()
                    + size_of::<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>()
                    + MAX_ATTACHMENTS * size_of::<DiagnosticPayloadVictimExpectation>()
            }
            Self::PayloadPreparation => {
                size_of::<DiagnosticPayloadDiscardPreparation>()
                    + size_of::<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>()
                    + MAX_ATTACHMENTS * size_of::<DiagnosticPayloadVictimObservation>()
            }
            Self::PayloadIntent => {
                size_of::<payload_discard::intent::PayloadIntent>()
                    + size_of::<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>()
                    + MAX_ATTACHMENTS * size_of::<payload_discard::intent::RetainedVictim>()
                    + DIAGNOSTIC_IO_BYTES
            }
            Self::MigrationIntent => {
                size_of::<summary_migration::MigrationIntent>() + DIAGNOSTIC_IO_BYTES
            }
        }
    }
}
