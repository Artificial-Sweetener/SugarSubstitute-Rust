// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate checked kernel peer facts from entry shape, commands and launch authority.
use crate::{
    entry_preparation::{EntryPreparationError, EntryRequestInput},
    native::NativeObservationError,
};
use std::fmt;
use sugarsubstitute_contracts::{
    coordinator_entry::{
        EntryConnectionId, EntryLaunchRequest, EntryNativeIdentityFacts, EntryShapeError,
        MAX_ENTRY_REQUEST_ALLOCATION_BYTES,
    },
    execution::{AdmissionError, CounterDeadline},
    lifecycle::{CoordinatorEpoch, NamespaceId},
};

/// Fixed maximum number of simultaneous native entry connections.
pub const MAX_ENTRY_CONNECTIONS: usize = 64;
/// Fixed maximum number of pending entry observations.
pub const MAX_ENTRY_OBSERVATIONS: usize = 512;
/// Maximum complete entry frame, including the 120-byte protocol header.
pub const MAX_ENTRY_FRAME_BYTES: usize = 65_536;
/// Known server storage charged before allocating duplex buffers and replay records.
/// Native operating-system allocations remain outside this measured storage.
pub const MIN_ENTRY_SERVER_BYTES: u64 = 16 * 1024 * 1024;
/// Known client storage charged before constructing native connection ownership.
pub const MIN_ENTRY_CLIENT_BYTES: u64 = 192 * 1024;

/// Retain inert expected identities before any server endpoint or native privilege exists.
#[derive(Debug)]
pub struct CoordinatorEntryServerInput {
    facts: EntryNativeIdentityFacts,
}
impl CoordinatorEntryServerInput {
    /// Retain validated shape; the worker verifies actual account, session and image.
    pub fn new(facts: EntryNativeIdentityFacts) -> Result<Self, EntryShapeError> {
        Ok(Self { facts })
    }
    /// Borrow matching facts, granting no namespace, election or launch authority.
    pub const fn facts(&self) -> &EntryNativeIdentityFacts {
        &self.facts
    }
}

/// Retain one inert request and its immutable original cross-process setup deadline.
/// Expected identities and a candidate epoch provide matching facts, not native authority.
pub struct CoordinatorEntryClientInput {
    pub(crate) facts: EntryNativeIdentityFacts,
    pub(super) candidate: Option<CoordinatorEpoch>,
    pub(super) request: EntryRequestInput,
    pub(super) source_setup: CounterDeadline,
}
impl CoordinatorEntryClientInput {
    /// Validate owned request capacity before native admission; preserve every original byte.
    /// The source deadline and frequency are checked against actual native counter facts later.
    pub fn new(
        facts: EntryNativeIdentityFacts,
        candidate: Option<CoordinatorEpoch>,
        request: EntryLaunchRequest,
        source_setup: CounterDeadline,
    ) -> Result<Self, EntryShapeError> {
        Self::from_owned(facts, candidate, request.into(), source_setup)
    }
    /// Retain complete prepared request data with its unchanged original custody.
    /// Native origin and deadline checks occur at actual client admission.
    /// # Errors
    /// Rejects Domain dynamic capacity above the unchanged 100KiB policy.
    pub fn from_owned(
        facts: EntryNativeIdentityFacts,
        candidate: Option<CoordinatorEpoch>,
        request: EntryRequestInput,
        source_setup: CounterDeadline,
    ) -> Result<Self, EntryShapeError> {
        if request.request().retained_bytes() > MAX_ENTRY_REQUEST_ALLOCATION_BYTES {
            return Err(EntryShapeError::Capacity);
        }
        Ok(Self { facts, candidate, request, source_setup })
    }
}
impl fmt::Debug for CoordinatorEntryClientInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("CoordinatorEntryClientInput").finish_non_exhaustive()
    }
}

/// Kernel peer metadata minted privately after actual connection credential checks.
/// Cached facts authorize neither process control nor backend, profile or UI access.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedEntryPeer {
    pub(super) connection: EntryConnectionId,
    pub(super) facts: EntryNativeIdentityFacts,
    pub(super) pid: u32,
    pub(super) birth: u64,
}
impl VerifiedEntryPeer {
    /// Return the connection whose actual kernel identity was inspected.
    pub const fn connection(self) -> EntryConnectionId {
        self.connection
    }
    /// Return installation/account matching facts, without their retained native proof.
    pub const fn namespace(self) -> NamespaceId {
        self.facts.namespace()
    }
    /// Return the actual kernel process identifier; it is never a process capability.
    pub const fn process_id(self) -> u32 {
        self.pid
    }
    /// Return actual process creation time in 100ns units since January 1, 1601 UTC.
    pub const fn creation_time_100ns(self) -> u64 {
        self.birth
    }
    /// Borrow the exact bounded primary-token account SID observed at authentication.
    pub fn account_sid(&self) -> &[u8] {
        self.facts.account_sid()
    }
    /// Return the authenticated Windows session identity.
    pub const fn session_id(self) -> u32 {
        self.facts.session_id()
    }
    /// Return actual pinned executable file identity: volume serial and 128-bit file ID.
    pub const fn host_image_identity(self) -> [u8; 24] {
        self.facts.host_image_identity()
    }
}
impl fmt::Debug for VerifiedEntryPeer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedEntryPeer")
            .field("connection", &self.connection)
            .finish_non_exhaustive()
    }
}

/// Reject a transport command without replacing another original bounded operation.
#[derive(Clone)]
pub enum EntryTransportError {
    /// The session or connection is closing; actual native retirement is separate.
    Closed,
    /// Its bounded command, buffer, loan or observation slot remains occupied.
    Busy,
    /// Operation identity is zero, reused, decreasing or exhausted.
    InvalidOperation,
    /// Absolute command deadline is expired or exceeds its original probe/ancestor.
    InvalidDeadline,
    /// Preparation, discovery or activation does not permit this command.
    InvalidPhase,
    /// Current kernel peer, nonce, namespace, epoch or request correlation differs.
    Authentication,
    /// The bounded frame has an unsupported schema or invalid shape.
    Malformed,
    /// Preserve the authoritative domain validator's original bounded shape rejection.
    Shape(EntryShapeError),
    /// Preserve original preparation control and its unchanged whole custody.
    Preparation(EntryPreparationError),
    /// Preserve actual preparation-related operation data and bounded post causes
    /// together with the unchanged whole owner, including cloned escaped errors.
    PreparedRetirement(super::EntryPreparedRetirement),
    /// A fixed connection or replay inventory cannot accept another entry.
    Capacity,
    /// Preserve the original central/native clock failure.
    Clock(NativeObservationError),
    /// An actual write found its requester disconnected before native acceptance.
    /// The original cause remains in checked connection retirement observations.
    PeerDisconnected(NativeObservationError),
    /// Preserve original native failure and sticky thread identity restoration facts.
    Native {
        /// Original owned cause, including bounded cleanup causes and owner panic.
        cause: NativeObservationError,
        /// Actual failed identity restoration requires whole-process shutdown.
        identity_unrestored: bool,
    },
    /// Preserve an original command/protocol failure when actual native cleanup also fails.
    Retirement {
        /// Original typed failure; cleanup never replaces its classification or cause.
        original: Box<EntryTransportError>,
        /// First actual cleanup cause, with bounded subsequent cleanup causes.
        cleanup: NativeObservationError,
        /// Sticky failed identity restoration fact across all retained causes.
        identity_unrestored: bool,
    },
}
impl EntryTransportError {
    /// Report actual failed thread identity restoration without classifying cause text.
    pub fn requires_process_shutdown(&self) -> bool {
        if let Self::PreparedRetirement(cause) = self {
            return cause.requires_process_shutdown();
        }
        matches!(
            self,
            Self::Native { identity_unrestored: true, .. }
                | Self::Retirement { identity_unrestored: true, .. }
        )
    }
    /// Borrow an unpaired native/clock cause; paired sources use their nonclone views.
    /// Paired observations retain underlying primary via Error::source and never project
    /// the cloneable native wrapper independently of original preparation custody.
    pub fn native_cause(&self) -> Option<&NativeObservationError> {
        match self {
            Self::PreparedRetirement(_) => None,
            Self::Clock(cause) | Self::PeerDisconnected(cause) | Self::Native { cause, .. } => {
                Some(cause)
            }
            Self::Retirement { original, cleanup, .. } => match original.native_cause() {
                Some(cause) => Some(cause),
                None => Some(cleanup),
            },
            _ => None,
        }
    }
    /// Preserve supervised panic classification independently of native cause text.
    pub fn contains_owner_panic(&self) -> bool {
        match self {
            Self::PreparedRetirement(cause) => cause.contains_owner_panic(),
            Self::Preparation(cause) => matches!(
                cause.cause(),
                Some(crate::entry_preparation::EntryPreparationInputError::Control(
                    crate::entry_preparation::EntryPreparationControlCause::ProviderPanic(_)
                ))
            ),
            Self::Retirement { original, cleanup, .. } => {
                original.contains_owner_panic() || cleanup.contains_owner_panic()
            }
            other => other.native_cause().is_some_and(NativeObservationError::contains_owner_panic),
        }
    }
    pub(crate) fn native(cause: NativeObservationError) -> Self {
        Self::Native { cause, identity_unrestored: false }
    }
    pub(crate) fn retain_cleanup(&mut self, next: Self) {
        if let Self::PreparedRetirement(retained) = self {
            retained.retain(next);
            return;
        }
        let owner = match &next {
            Self::Preparation(cause) => cause.retained_owner().cloned(),
            _ => match self {
                Self::Preparation(cause) => cause.retained_owner().cloned(),
                _ => None,
            },
        };
        if let Some(owner) = owner {
            let original = std::mem::replace(self, Self::Closed);
            *self = Self::prepared(original, Some(next), Some(&owner));
            return;
        }
        let fatal = self.requires_process_shutdown() || next.requires_process_shutdown();
        let next = match next {
            Self::Clock(cause) | Self::Native { cause, .. } => cause,
            other => NativeObservationError::new("entry_cleanup_invariant", other),
        };
        match self {
            Self::Native { cause, identity_unrestored } => {
                *identity_unrestored = fatal;
                cause.retain_cleanup(next);
            }
            Self::Retirement { cleanup, identity_unrestored, .. } => {
                *identity_unrestored = fatal;
                cleanup.retain_cleanup(next);
            }
            original => {
                let first = std::mem::replace(original, Self::Closed);
                *original = Self::Retirement {
                    original: Box::new(first),
                    cleanup: next,
                    identity_unrestored: fatal,
                };
            }
        }
    }
}
impl fmt::Debug for EntryTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native { cause, identity_unrestored } => formatter
                .debug_struct("EntryTransportError::Native")
                .field("stage", &cause.stage())
                .field("identity_unrestored", identity_unrestored)
                .finish_non_exhaustive(),
            Self::PeerDisconnected(cause) => formatter
                .debug_struct("EntryTransportError::PeerDisconnected")
                .field("stage", &cause.stage())
                .finish_non_exhaustive(),
            Self::Clock(cause) => formatter
                .debug_struct("EntryTransportError::Clock")
                .field("stage", &cause.stage())
                .finish_non_exhaustive(),
            Self::Retirement { original, cleanup, identity_unrestored } => formatter
                .debug_struct("EntryTransportError::Retirement")
                .field("original", original)
                .field("cleanup_stage", &cleanup.stage())
                .field("identity_unrestored", identity_unrestored)
                .finish_non_exhaustive(),
            Self::Closed => formatter.write_str("Closed"),
            Self::Busy => formatter.write_str("Busy"),
            Self::InvalidOperation => formatter.write_str("InvalidOperation"),
            Self::InvalidDeadline => formatter.write_str("InvalidDeadline"),
            Self::InvalidPhase => formatter.write_str("InvalidPhase"),
            Self::Authentication => formatter.write_str("Authentication"),
            Self::Malformed => formatter.write_str("Malformed"),
            Self::Shape(cause) => formatter.debug_tuple("Shape").field(cause).finish(),
            Self::Preparation(cause) => formatter.debug_tuple("Preparation").field(cause).finish(),
            Self::PreparedRetirement(cause) => {
                formatter.debug_tuple("PreparedRetirement").field(cause).finish()
            }
            Self::Capacity => formatter.write_str("Capacity"),
        }
    }
}
impl fmt::Display for EntryTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Coordinator entry transport operation rejected")
    }
}
impl std::error::Error for EntryTransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Retirement { original, .. } => Some(original.as_ref()),
            Self::PreparedRetirement(cause) => {
                cause.original().map(|cause| cause as &(dyn std::error::Error + 'static))
            }
            Self::Shape(cause) => Some(cause),
            Self::Preparation(cause) => Some(cause),
            other => other.native_cause().map(|cause| cause as &(dyn std::error::Error + 'static)),
        }
    }
}
/// Reject admission before creating an endpoint, native wait or authenticated peer.
pub enum EntryTransportAdmissionError {
    /// Preserve the actual central execution reservation rejection.
    Admission(AdmissionError),
    /// Preserve the original actual QPC/central epoch conversion failure.
    Clock(NativeObservationError),
    /// Reject bounded inert input shape before native effects.
    Input(EntryShapeError),
    /// Reject current original preparation before source counter or native effects.
    Preparation(EntryPreparationError),
    /// Preserve a genuine original operation with its retained whole custody.
    Prepared(EntryTransportError),
    /// The persistent session specification or any ancestor supplied a deadline.
    ConflictingDeadline,
}
impl fmt::Debug for EntryTransportAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EntryTransportAdmissionError { redacted }")
    }
}
impl fmt::Display for EntryTransportAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Coordinator entry transport admission rejected")
    }
}
impl std::error::Error for EntryTransportAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(cause) => Some(cause),
            Self::Clock(cause) => Some(cause),
            Self::Input(cause) => Some(cause),
            Self::Prepared(cause) => Some(cause),
            Self::Preparation(cause) => Some(cause),
            Self::ConflictingDeadline => None,
        }
    }
}

/// Observe checked session retirement independently of process or family settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryTransportReport {
    /// Number of actually closed connection lifetimes; IDs never wrap or repeat.
    pub retired_connections: u64,
    /// Number of actual authenticated Launch observations emitted by this transport.
    pub authenticated_launches: u64,
    /// Number of actual completed native writes; peer processing remains separate.
    pub completed_writes: u64,
}
