// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe inert source matching data and preserve portable-reader failure causes.
use super::*;
use std::{error::Error, fmt, io};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryClaimError};

/// Categorize a passive-history refusal without implying native recovery permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortableIncidentFailureCode {
    /// The original source or export sink returned a retained I/O cause.
    Source,
    /// The existing incident codec rejected actual record bytes.
    Codec,
    /// Inventory, content or original memory admission exceeded a fixed bound.
    Capacity,
    /// Data belongs to a namespace other than the verified preserved source.
    ForeignNamespace,
    /// Unknown, quarantined or unsupported content cannot qualify committed history.
    UnsupportedRecord,
    /// An unfinished transaction or unassociated artifact cannot become history.
    Uncommitted,
    /// Pinned source matching data changed or no longer names the expected object.
    ChangedSource,
    /// Complete streamed content disagrees with its committed hash or byte length.
    WrongDigest,
    /// Required committed metadata or an attachment is absent.
    MissingContent,
    /// Duplicate logical records or opaque source identifiers are ambiguous.
    Duplicate,
    /// Cancellation was requested on the original execution context.
    Cancelled,
    /// The original job deadline elapsed.
    Deadline,
}

#[derive(Debug)]
enum Cause {
    Source(io::Error),
    Codec(Box<IncidentStoreError>),
    Memory(MemoryClaimError),
    Allocation(std::collections::TryReserveError),
}

/// Preserve the first typed refusal and its original cause with admitted decode memory.
#[derive(Debug)]
pub struct PortableIncidentError {
    code: PortableIncidentFailureCode,
    cause: Option<Cause>,
    memory: Option<MemoryClaim>,
}
impl PortableIncidentError {
    /// Preserve a source/sink adapter's actual I/O cause without changing its meaning.
    pub fn source(cause: io::Error) -> Self {
        Self {
            code: PortableIncidentFailureCode::Source,
            cause: Some(Cause::Source(cause)),
            memory: None,
        }
    }
    /// Explain a pure bounded refusal; this grants no file or operation authority.
    pub const fn rejected(code: PortableIncidentFailureCode) -> Self {
        Self { code, cause: None, memory: None }
    }
    /// Observe the categorical failure while its complete original cause stays owned.
    pub const fn code(&self) -> PortableIncidentFailureCode {
        self.code
    }
    pub(super) fn codec(cause: IncidentStoreError) -> Self {
        Self {
            code: PortableIncidentFailureCode::Codec,
            cause: Some(Cause::Codec(Box::new(cause))),
            memory: None,
        }
    }
    pub(super) fn memory(cause: MemoryClaimError) -> Self {
        Self {
            code: PortableIncidentFailureCode::Capacity,
            cause: Some(Cause::Memory(cause)),
            memory: None,
        }
    }
    pub(super) fn allocation(cause: std::collections::TryReserveError) -> Self {
        Self {
            code: PortableIncidentFailureCode::Capacity,
            cause: Some(Cause::Allocation(cause)),
            memory: None,
        }
    }
    pub(super) fn retain(mut self, memory: MemoryClaim) -> Self {
        self.memory = Some(memory);
        self
    }
}
impl fmt::Display for PortableIncidentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Preserved incident history rejected: {:?}", self.code)
    }
}
impl Error for PortableIncidentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.cause.as_ref()? {
            Cause::Source(cause) => Some(cause),
            Cause::Codec(cause) => Some(cause.as_ref()),
            Cause::Memory(cause) => Some(cause),
            Cause::Allocation(cause) => Some(cause),
        }
    }
}

/// Match a C-owned verified preserved tree; these values grant no read or write capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortableIncidentProvenance {
    namespace: NamespaceId,
    tree_sha256: [u8; 32],
}
impl PortableIncidentProvenance {
    /// Record the actual selected source namespace and verified full preserved-tree digest.
    /// # Errors
    /// Rejects a zero digest. The actual source adapter must independently retain provenance.
    pub fn new(
        namespace: NamespaceId,
        tree_sha256: [u8; 32],
    ) -> Result<Self, PortableIncidentError> {
        if tree_sha256 == [0; 32] {
            return Err(PortableIncidentError::rejected(
                PortableIncidentFailureCode::ChangedSource,
            ));
        }
        Ok(Self { namespace, tree_sha256 })
    }
    /// Observe the original namespace without manufacturing authority over it.
    pub const fn namespace(self) -> NamespaceId {
        self.namespace
    }
    /// Observe preserved-tree content provenance, not a live native permission.
    pub const fn tree_sha256(self) -> [u8; 32] {
        self.tree_sha256
    }
}

/// Identify one entry in this retained source session, not a native handle or pathname.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortableIncidentEntryId(u16);
impl PortableIncidentEntryId {
    /// Admit one ordinal within the existing diagnostic scan bound.
    /// # Errors
    /// Rejects an ordinal outside the 4,096-entry inventory.
    pub fn new(value: u16) -> Result<Self, PortableIncidentError> {
        if usize::from(value) >= DIAGNOSTIC_SCAN_BOUND {
            return Err(PortableIncidentError::rejected(PortableIncidentFailureCode::Capacity));
        }
        Ok(Self(value))
    }
    /// Observe this session-local ordinal without gaining native access.
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Carry bounded actual inventory matching data while source custody stays in its adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortableIncidentEntry {
    pub(super) id: PortableIncidentEntryId,
    pub(super) class: DiagnosticEntryClass,
    pub(super) bytes: u64,
}
impl PortableIncidentEntry {
    /// Record a protected source observation without interpreting its filename as a key.
    /// # Errors
    /// Rejects empty files and objects larger than the existing 256-MiB payload bound.
    pub fn new(
        id: PortableIncidentEntryId,
        class: DiagnosticEntryClass,
        bytes: u64,
    ) -> Result<Self, PortableIncidentError> {
        if bytes == 0 || bytes > MAX_PAYLOAD_BYTES {
            return Err(PortableIncidentError::rejected(PortableIncidentFailureCode::Capacity));
        }
        Ok(Self { id, class, bytes })
    }
    /// Observe the matching ordinal used by this actual retained reader.
    pub const fn id(self) -> PortableIncidentEntryId {
        self.id
    }
    /// Observe the native owner's candidate class, awaiting semantic validation.
    pub const fn class(self) -> DiagnosticEntryClass {
        self.class
    }
    /// Observe actual complete byte length, independently rechecked while streaming.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

/// Describe one completed verified export; no acknowledgement or mutation is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortableIncidentExport {
    pub(super) incident: IncidentKey,
    pub(super) kind: AttachmentKind,
    pub(super) bytes: u64,
    pub(super) sha256: [u8; 32],
}
impl PortableIncidentExport {
    /// Observe original incident attribution, including its historical namespace.
    pub const fn incident(self) -> IncidentKey {
        self.incident
    }
    /// Observe the exported allowlisted attachment kind.
    pub const fn kind(self) -> AttachmentKind {
        self.kind
    }
    /// Observe actual complete exported bytes.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
    /// Observe the verified complete exported content hash.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}
