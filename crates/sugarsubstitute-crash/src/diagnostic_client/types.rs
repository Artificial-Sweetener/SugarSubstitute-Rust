// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep current session/operation facts distinct from complete historical subjects.
use super::{DiagnosticCommand, DiagnosticWireError as E, command_codec};
use std::num::NonZeroU64;
use sugarsubstitute_contracts::incident::*;
/// Positive request identity with a nonwrapping connection lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticRequestId(NonZeroU64);
impl DiagnosticRequestId {
    /// Validate without admitting work; zero rejects.
    pub fn new(value: u64) -> Result<Self, E> {
        NonZeroU64::new(value).map(Self).ok_or(E::Identity)
    }
    /// Observe plain correlation.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Original signed Windows counter facts, never interpreted as local Moment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticDeadline {
    ticks: i64,
    frequency: i64,
}
impl DiagnosticDeadline {
    /// Validate positive original counter facts; no clock is read.
    pub const fn new(ticks: i64, frequency: i64) -> Result<Self, E> {
        if ticks <= 0 || frequency <= 0 { Err(E::Time) } else { Ok(Self { ticks, frequency }) }
    }
    /// Original absolute ticks.
    pub const fn ticks(self) -> i64 {
        self.ticks
    }
    /// Original ticks per second.
    pub const fn frequency(self) -> i64 {
        self.frequency
    }
}
/// Complete potentially historical incident subject and CAS revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticIncident {
    /// Inert historical/current subject; current native authority stays separate.
    pub key: IncidentKey,
    /// Exact expected content revision.
    pub revision: IncidentRevision,
}
/// Exact context carried without inventing missing operation/UTC/deadline fields.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticContext {
    /// Current admitted logical operation, when the command is finite work.
    pub operation: Option<LogicalStoreOperationKey>,
    /// Complete subject/revision, when a command concerns an incident.
    pub incident: Option<DiagnosticIncident>,
    /// Original counter deadline; passive reporter observations have none.
    pub deadline: Option<DiagnosticDeadline>,
    /// Separately injected UTC, required for storage/symbol operations.
    pub at: Option<IncidentUtc>,
}
/// Fully decoded borrowed frame; retaining it never grants adapter privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticFrame<'a> {
    /// Exact current native-session matching facts.
    pub key: DiagnosticKey,
    /// Checked positive request identity.
    pub request: DiagnosticRequestId,
    /// Original context with full historical identity.
    pub context: DiagnosticContext,
    /// Closed typed command and borrowed payload.
    pub command: super::DiagnosticCommand<'a>,
}
/// Checked incoming publication demand, not a storage reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticPublicationDemand {
    bytes: u64,
    groups: u32,
    slots: u32,
    headroom: u64,
}
impl DiagnosticPublicationDemand {
    /// Apply actual08 literal construction rules before encoding or decoding.
    pub const fn new(bytes: u64, groups: u32, slots: u32, headroom: u64) -> Result<Self, E> {
        if groups > 1
            || slots > 1
            || (bytes == 0) != (groups == 0)
            || headroom < 131_072
            || headroom > 268_435_456
        {
            return Err(E::Capacity);
        }
        match bytes.checked_add(headroom) {
            Some(sum) if sum <= 268_435_456 => Ok(Self { bytes, groups, slots, headroom }),
            _ => Err(E::Capacity),
        }
    }
    /// Prepared incoming payload bytes.
    pub const fn payload_bytes(self) -> u64 {
        self.bytes
    }
    /// Number of new immutable payload generations.
    pub const fn payload_groups(self) -> u32 {
        self.groups
    }
    /// New Summary slots, checked against actual predecessor by repository.
    pub const fn summary_slots(self) -> u32 {
        self.slots
    }
    /// Prepared staging peak retained through actual retirement.
    pub const fn staging_headroom(self) -> u64 {
        self.headroom
    }
}
/// Exact raw transfer descriptor; each borrowed chunk has at most60KiB.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticChunk<'a> {
    /// Closed transfer identity; prevents Summary/view/attachment confusion.
    pub kind: DiagnosticTransferKind,
    /// Original total bytes, bounded by its command owner.
    pub total: u64,
    /// Exact contiguous offset; arbitrary seeks are rejected by transfer owner.
    pub offset: u64,
    /// Original complete digest.
    pub sha256: [u8; 32],
    /// Raw bytes borrowed from the actual charged frame.
    pub bytes: &'a [u8],
}
/// Expected Summary predecessor; copied facts are not a CAS permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticExpectedSummary {
    /// Actual repository must independently verify absence.
    Absent,
    /// Exact predecessor revision/digest.
    Exact {
        /// Positive predecessor revision.
        revision: IncidentRevision,
        /// Complete canonical Summary digest.
        sha256: [u8; 32],
    },
}

/// Closed byte-stream owner; no kind grants read/write authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticTransferKind {
    /// Canonical bounded persisted Summary.
    Summary,
    /// Complete bounded redacted UTF8 native presentation.
    View,
    /// One exact bounded attachment kind.
    Attachment(AttachmentKind),
}

/// Closed original request kind carried by a bounded result envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticRequestKind(u8);
impl DiagnosticRequestKind {
    /// Capture a real command's closed kind; results cannot recursively reply to results.
    /// # Errors
    /// Rejects a result envelope as an original request.
    pub fn of(command: DiagnosticCommand<'_>) -> Result<Self, E> {
        Self::from_code(command_codec::code(command))
    }
    pub(super) const fn from_code(code: u8) -> Result<Self, E> {
        if code >= 1 && code <= 30 { Ok(Self(code)) } else { Err(E::Schema) }
    }
    pub(super) const fn code(self) -> u8 {
        self.0
    }
}

/// Closed finite-operation failures; none imply source/family/native settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticOperationFailure {
    /// Actual execution/native admission rejected.
    Admission,
    /// Actual cancellation terminal, not stopped-process proof.
    Cancelled,
    /// Original deadline expired, without renewal or settlement.
    Deadline,
    /// Actual bounded resource/storage capacity unavailable.
    Capacity,
    /// Actual identity/ownership mismatch.
    Identity,
    /// Protected repository/storage effect failed.
    Storage,
    /// Actual durable recovery remains failed/uncertain.
    Recovery,
    /// Actual capture source unavailable/failed.
    Capture,
    /// Actual trusted symbol association/processing failed.
    Symbols,
    /// Actual selected export failed without acknowledgement.
    Export,
    /// Actual native presentation failed.
    Presentation,
    /// Actual checked retirement remains uncertain.
    Retirement,
}
