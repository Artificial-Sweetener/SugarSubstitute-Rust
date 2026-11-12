// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe bounded diagnostic evidence without paths, attachment data or native authority.
use super::{DumpId, IncidentError, MAX_ATTACHMENTS, MAX_PAYLOAD_BYTES, MAX_TEXT_ATTACHMENT_BYTES};
use crate::startup::StartupFailureCode;

/// Mechanism supplied by an authoritative evidence boundary; never inferred from a dump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureEvidence {
    /// Exit was unexplained; no confirmed fault mechanism is supplied.
    Unexplained,
    /// Startup owner supplied an explicit failure category.
    Startup(StartupFailureCode),
    /// A Rust panic boundary supplied panic evidence.
    RustPanic,
    /// An authoritative boundary supplied abort evidence.
    Abort,
    /// Actual exception evidence includes its code and fault address, not a full stack.
    NativeFault {
        /// Platform exception code, retained exactly.
        code: u32,
        /// Actual exception address in the faulting process's address space.
        address: u64,
    },
}
/// Why native evidence is currently unavailable, without asserting absence of a crash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureAbsence {
    /// Registration was unavailable or degraded.
    HandlerUnavailable,
    /// A previously registered handler actually exited or its observation failed.
    HandlerLost,
    /// No finished report was found for the exact subject.
    Missing,
    /// The report remained in progress/busy.
    Busy,
    /// The original capture-operation deadline elapsed.
    DeadlineElapsed,
    /// Report identity/build/subject disagreed.
    WrongIdentity,
    /// Evidence could not be validated.
    Corrupt,
    /// Evidence exceeded admitted storage/read bounds.
    Oversized,
}
/// Native capture availability, independent of M1's exit attribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureEvidence {
    /// The capture owner has not delivered a terminal availability fact.
    Awaiting,
    /// Categorical unavailability; missing dump does not prove a clean exit.
    Unavailable(CaptureAbsence),
    /// The adapter observed a finished exact-identity report, not merely a file.
    Finished(DumpId),
}
/// Allowlisted artifact purpose; this enum contains no caller-selected filename.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentKind {
    /// Bounded startup diagnostic tail.
    Startup,
    /// Bounded fault diagnostic tail.
    Fault,
    /// Bounded runtime context.
    Runtime,
    /// Bounded structured log tail.
    Log,
    /// Bounded actual failing-frame symbol report.
    SymbolizedFault,
    /// Opaque native memory dump, never described as sanitized text.
    Minidump,
}
/// Size and digest facts about one retained artifact, without read/export authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentFacts {
    kind: AttachmentKind,
    retained_bytes: u64,
    original_bytes: u64,
    omitted_bytes: u64,
    sha256: [u8; 32],
}
impl AttachmentFacts {
    /// Validate nonempty bounded content and explicit source/omission facts.
    /// # Errors
    /// Rejects oversized text/payload, omission exceeding source bytes, and
    /// truncated minidumps. Stored text lengths include markers/redaction output;
    /// omitted source bytes are supplied separately rather than subtracting them.
    pub fn new(
        kind: AttachmentKind,
        retained_bytes: u64,
        original_bytes: u64,
        omitted_bytes: u64,
        sha256: [u8; 32],
    ) -> Result<Self, IncidentError> {
        if retained_bytes == 0
            || retained_bytes > MAX_PAYLOAD_BYTES
            || omitted_bytes > original_bytes
            || (kind == AttachmentKind::Minidump
                && (original_bytes != retained_bytes || omitted_bytes != 0))
            || (kind != AttachmentKind::Minidump && retained_bytes > MAX_TEXT_ATTACHMENT_BYTES)
        {
            return Err(IncidentError::InvalidEvidence);
        }
        Ok(Self { kind, retained_bytes, original_bytes, omitted_bytes, sha256 })
    }
    /// Return the allowlisted artifact purpose.
    pub const fn kind(self) -> AttachmentKind {
        self.kind
    }
    /// Return retained bytes, including text marker bytes.
    pub const fn retained_bytes(self) -> u64 {
        self.retained_bytes
    }
    /// Return original source bytes, before bounded text omission.
    pub const fn original_bytes(self) -> u64 {
        self.original_bytes
    }
    /// Return explicitly omitted source bytes, independent of marker/redaction lengths.
    /// Minidumps always return zero.
    pub const fn omitted_bytes(self) -> u64 {
        self.omitted_bytes
    }
    /// Borrow the exact expected content digest; this does not verify content itself.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}
/// Fixed-capacity, unique-kind artifact inventory below the total payload ceiling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentInventory {
    entries: Box<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>,
    total_bytes: u64,
    length: usize,
}
impl AttachmentInventory {
    /// Retain a nonempty bounded inventory without cloning artifact data.
    /// # Errors
    /// Rejects empty/oversized inventories, repeated kinds and aggregate overflow/cap.
    pub fn new(input: &[AttachmentFacts]) -> Result<Self, IncidentError> {
        if input.is_empty() || input.len() > MAX_ATTACHMENTS {
            return Err(IncidentError::InvalidEvidence);
        }
        let mut entries = [None; MAX_ATTACHMENTS];
        let mut total_bytes = 0_u64;
        for (index, fact) in input.iter().enumerate() {
            if input[..index].iter().any(|prior| prior.kind == fact.kind) {
                return Err(IncidentError::InvalidEvidence);
            }
            total_bytes = total_bytes
                .checked_add(fact.retained_bytes)
                .filter(|bytes| *bytes <= MAX_PAYLOAD_BYTES)
                .ok_or(IncidentError::InvalidEvidence)?;
            entries[index] = Some(*fact);
        }
        Ok(Self { entries: Box::new(entries), total_bytes, length: input.len() })
    }
    /// Borrow retained facts in the supplied stable order.
    pub fn entries(&self) -> impl Iterator<Item = &AttachmentFacts> {
        self.entries[..self.length].iter().flatten()
    }
    /// Return the exact aggregate retained content bytes.
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}
/// Explicit reason retained payload was discarded while its summary remains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadDiscard {
    /// Acknowledged age/count retention reclaimed the payload.
    AcknowledgedRetention,
    /// Total byte or full-incident pressure reclaimed the payload.
    StoragePressure,
    /// A single source exceeded the maximum admitted payload.
    Oversized,
    /// Explicit user disposition requested payload removal.
    UserRequested,
}
/// Explicit reason and source byte count retained after payload reclamation/rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscardedPayload {
    reason: PayloadDiscard,
    source_bytes: u64,
}
impl DiscardedPayload {
    /// Retain a nonempty discarded source's size without claiming checked deletion.
    /// # Errors
    /// Returns `InvalidEvidence` for zero; absent evidence uses `Unavailable` instead.
    pub fn new(reason: PayloadDiscard, source_bytes: u64) -> Result<Self, IncidentError> {
        if source_bytes == 0 {
            return Err(IncidentError::InvalidEvidence);
        }
        Ok(Self { reason, source_bytes })
    }
    /// Return the explicit categorical discard reason.
    pub const fn reason(self) -> PayloadDiscard {
        self.reason
    }
    /// Return observed discarded/rejected source bytes, which may exceed admission limits.
    pub const fn source_bytes(self) -> u64 {
        self.source_bytes
    }
}
/// Summary-level retained-artifact disposition; no filesystem operations are implied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PayloadDisposition {
    /// No retained artifact was available for this incident.
    Unavailable,
    /// Validated retained artifacts remain referenced by the summary.
    Retained(AttachmentInventory),
    /// A durable explicit discard disposition is supplied as a fact.
    Discarded(DiscardedPayload),
    /// Referenced evidence is corrupt/missing; recovery preserves that observation.
    Quarantined,
}
