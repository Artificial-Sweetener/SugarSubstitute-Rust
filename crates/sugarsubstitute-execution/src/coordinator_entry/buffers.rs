// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep immutable authenticated payload allocations charged until passive loans return.
use super::*;
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryLaunchRequestView, EntryRequestId},
    execution::{CounterDeadline, MemoryClaim, MemoryLease},
};
pub(super) struct Buffer {
    pub(super) bytes: Box<[u8]>,
    _claim: MemoryClaim,
}
impl Buffer {
    pub(super) fn new(memory: &MemoryLease) -> Result<Self, EntryTransportError> {
        let claim = memory.claim(MAX_ENTRY_FRAME_BYTES as u64).map_err(|cause| {
            EntryTransportError::native(NativeObservationError::new("entry_frame_storage", cause))
        })?;
        Ok(Self { bytes: vec![0; MAX_ENTRY_FRAME_BYTES].into_boxed_slice(), _claim: claim })
    }
}
pub(super) struct ReceivedEntryFrame {
    pub(super) buffer: Option<Buffer>,
    pub(super) length: usize,
    pub(super) index: usize,
    pub(super) shared: Arc<Shared>,
}
impl ReceivedEntryFrame {
    fn bytes(&self) -> &[u8] {
        self.buffer.as_ref().map_or(&[], |buffer| &buffer.bytes[..self.length])
    }
}
impl Drop for ReceivedEntryFrame {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            let mut state = lock(&self.shared.state);
            if let Some(connection) = state.connections.get_mut(self.index) {
                connection.returned = Some(buffer);
                connection.loan = false;
            }
            drop(state);
            self.shared.changed();
        }
    }
}
/// Own a checked Launch from an actual authenticated current kernel connection.
/// This transport proof grants no election, profile, child or process privilege.
pub struct AuthenticatedEntryLaunch {
    pub(super) frame: ReceivedEntryFrame,
    pub(super) request: EntryRequestId,
    pub(super) received: Moment,
    pub(super) until: Moment,
    pub(super) source: CounterDeadline,
    pub(super) peer: VerifiedEntryPeer,
}
impl AuthenticatedEntryLaunch {
    /// Return its immutable original source-counter-derived request identity.
    pub const fn request_id(&self) -> EntryRequestId {
        self.request
    }
    /// Decode the same immutable retained frame with the authoritative domain shape validators.
    /// Successful shape decoding must precede root admission or native effects.
    /// # Errors
    /// Returns the original typed decoder failure if retained bytes fail closed validation.
    pub fn view(&self) -> Result<EntryLaunchRequestView<'_>, EntryTransportError> {
        match protocol::decode(self.frame.bytes())?.payload {
            protocol::DecodedPayload::Launch(view) => Ok(view),
            _ => Err(EntryTransportError::Malformed),
        }
    }
    /// Return the original central-clock receive sample in nanoseconds.
    pub const fn received_at(&self) -> Moment {
        self.received
    }
    /// Return the original conservatively translated and locally capped receive deadline.
    pub const fn until(&self) -> Moment {
        self.until
    }
    /// Return exact original native ticks and frequency without duration renewal.
    pub const fn source_deadline(&self) -> CounterDeadline {
        self.source
    }
    /// Borrow actual kernel facts for the root's separate retained authority validation.
    pub const fn peer(&self) -> &VerifiedEntryPeer {
        &self.peer
    }
}
impl std::fmt::Debug for AuthenticatedEntryLaunch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthenticatedEntryLaunch")
            .field("request", &self.request)
            .finish_non_exhaustive()
    }
}
