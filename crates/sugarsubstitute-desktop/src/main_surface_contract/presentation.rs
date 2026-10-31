// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain asynchronous full presentation snapshots independently of request credit.

use super::{SurfaceReply, SurfaceReplyError};
use sugarsubstitute_contracts::{
    canvas_surface::{CanvasSurfaceAddress, CanvasSurfacePresentationChange},
    execution::MemoryClaim,
    panel::PanelSession,
    startup::MainIdentity,
};

/// One bounded notification retains its own storage until actual native callback retirement.
/// It owns no ordinary transport ticket and cannot complete or replenish a request lane.
pub struct SurfacePresentation {
    identity: MainIdentity,
    address: CanvasSurfaceAddress,
    sequence: u64,
    payload: Vec<u8>,
    _claim: MemoryClaim,
}

impl SurfacePresentation {
    /// Storage covering the encoded snapshot, address and native command serialization.
    pub const STORAGE_BYTES: u64 = SurfaceReply::STORAGE_BYTES;

    /// Qualify a native full-state observation with the exact current main session.
    ///
    /// Callers retain attachment authority; this object supplies correlation and storage.
    /// # Errors
    /// Rejects inadequate storage, invalid native identity, or shared encoding failure.
    pub fn new(
        identity: MainIdentity,
        change: &CanvasSurfacePresentationChange,
        claim: MemoryClaim,
    ) -> Result<Self, SurfaceReplyError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(SurfaceReplyError::Storage);
        }
        let session = PanelSession::for_main(identity).map_err(SurfaceReplyError::Identity)?;
        let payload = change.encode(&session).map_err(SurfaceReplyError::Encode)?;
        Ok(Self {
            identity,
            address: change.address().clone(),
            sequence: change.sequence(),
            payload,
            _claim: claim,
        })
    }

    /// Return the native navigation that owns this observation.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Borrow the complete schema-validated asynchronous wire envelope.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Borrow the attachment correlation; it grants no native surface authority.
    pub fn address(&self) -> &CanvasSurfaceAddress {
        &self.address
    }

    /// Compare only exact native navigation and attachment identities for pending coalescing.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn same_target(&self, other: &Self) -> bool {
        self.identity == other.identity && self.address == other.address
    }

    /// Return the authoritative increasing sequence; pending snapshot delivery may skip values.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Destroy all encoded/address storage before returning the independently admitted claim.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn into_claim(self) -> MemoryClaim {
        let Self { payload, address, _claim, .. } = self;
        drop((payload, address));
        _claim
    }
}
