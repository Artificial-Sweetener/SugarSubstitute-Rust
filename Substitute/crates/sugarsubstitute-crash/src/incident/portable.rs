// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read preserved committed incidents without moving historical native authority.
//!
//! The source adapter owns actual protected files, provenance and checked retirement.
//! This owner validates data only. It never returns a writer, genesis, recovery permit
//! or native token and cannot publish an old namespace into the elected active store.
mod archives;
mod export;
mod records;
mod selection;
mod source;
mod types;
use super::{IncidentStoreError, StoredIncidentSummary};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim},
    incident::{AttachmentKind, IncidentKey, IncidentUtc, MAX_PAYLOAD_BYTES},
    lifecycle::NamespaceId,
};
use sugarsubstitute_supervision::diagnostic_storage::{
    DIAGNOSTIC_IO_BYTES, DIAGNOSTIC_SCAN_BOUND, DiagnosticEntryClass, DiagnosticObject,
};
pub use types::*;

/// Fixed passive metadata admission; payload bytes are streamed in 64-KiB chunks.
pub const PORTABLE_INCIDENT_HISTORY_BYTES: u64 = 16 * 1024 * 1024;

/// Borrow the actual protected preserved-copy reader, never a caller-controlled path.
///
/// The application adapter retains directory/file pins and verified selected-generation
/// provenance. Every method checks the same original execution context and current pinned
/// copy identity. Historical native IDs inside archived bytes are never read authority.
pub trait PortableIncidentReadSession {
    /// Observe the verified preserved-tree namespace and content provenance.
    fn provenance(&self) -> PortableIncidentProvenance;
    /// Borrow a complete stable inventory, at most the existing 4,096-entry bound.
    /// # Errors
    /// Preserves incomplete inventory, protection, changed-source and original control causes.
    fn inventory(
        &mut self,
        context: &JobContext,
    ) -> Result<&[PortableIncidentEntry], PortableIncidentError>;
    /// Read at most `output.len()` bytes at this entry's byte offset, maximum 64 KiB.
    /// # Errors
    /// Preserves actual same-file/protection/deadline failures. Zero means observed EOF.
    fn read_chunk(
        &mut self,
        entry: PortableIncidentEntryId,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, PortableIncidentError>;
    /// Compare an actual canonical source name with a key derived from strict record bytes.
    ///
    /// `false` means a genuine different canonical object; it grants no key association.
    /// # Errors
    /// Rejects changed current source custody or original execution control.
    fn bind_typed_object(
        &mut self,
        entry: PortableIncidentEntryId,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<bool, PortableIncidentError>;
    /// Revalidate all actual retained source identities and original provenance.
    /// # Errors
    /// Refuses replacement, mutation, foreign protection, cancellation or deadline.
    fn revalidate(&mut self, context: &JobContext) -> Result<(), PortableIncidentError>;
}

struct Payload {
    entry: PortableIncidentEntry,
    object: DiagnosticObject,
    sha256: [u8; 32],
}
struct HistoryData {
    summaries: Vec<StoredIncidentSummary>,
    payloads: Vec<Payload>,
    evictions: sugarsubstitute_contracts::incident::SummaryEvictions,
}

/// Borrow qualified passive data and its still-owned source session.
///
/// This view belongs to one finite read/export operation. Later UI requests open
/// a fresh actual pinned session and original job context; this view cannot renew
/// its deadline, transfer admission or serve as a durable UI-session cache.
///
/// Dropping this view does not claim that the adapter's native owners retired.
/// The caller must observe that adapter's checked closure. No active-store capability
/// can be constructed from this view, its provenance or its summary data.
pub struct PortableIncidentHistory<'a> {
    session: &'a mut dyn PortableIncidentReadSession,
    provenance: PortableIncidentProvenance,
    data: HistoryData,
    _memory: MemoryClaim,
    context: JobContext,
}
impl PortableIncidentHistory<'_> {
    /// Borrow validated historical summaries without changing their attribution or retention.
    pub fn summaries(&self) -> impl ExactSizeIterator<Item = &StoredIncidentSummary> {
        self.data.summaries.iter()
    }
    /// Observe the verified original namespace and inert preserved-tree provenance.
    pub const fn provenance(&self) -> PortableIncidentProvenance {
        self.provenance
    }
    /// Observe the original validated namespace ledger without renewing retention.
    pub const fn evictions(&self) -> sugarsubstitute_contracts::incident::SummaryEvictions {
        self.data.evictions
    }
}

/// Qualify a complete preserved committed record set without executing historical journals.
///
/// The expected namespace comes from the actual selected source owner, independently
/// of source-host correlation. Supported legacy Summary conversion is read-only and
/// uses the existing codec; original files are never rewritten. Completed migration
/// archives remain inert. Prelaunch origins, unfinished journals and unknown data refuse.
/// # Errors
/// Preserves strict codec, source custody, content, memory and original job-control causes.
pub fn project_portable_committed<'a>(
    session: &'a mut dyn PortableIncidentReadSession,
    expected_namespace: NamespaceId,
    now: IncidentUtc,
    context: &JobContext,
) -> Result<PortableIncidentHistory<'a>, PortableIncidentError> {
    let memory = context
        .memory()
        .claim(PORTABLE_INCIDENT_HISTORY_BYTES)
        .map_err(PortableIncidentError::memory)?;
    let provenance = session.provenance();
    let result = source::control(context).and_then(|()| {
        if provenance.namespace() != expected_namespace {
            return Err(PortableIncidentError::rejected(
                PortableIncidentFailureCode::ForeignNamespace,
            ));
        }
        session.revalidate(context)?;
        selection::qualify(session, provenance, now, context)
    });
    match result {
        Ok(data) => Ok(PortableIncidentHistory {
            session,
            provenance,
            data,
            _memory: memory,
            context: context.clone(),
        }),
        Err(cause) => Err(cause.retain(memory)),
    }
}
