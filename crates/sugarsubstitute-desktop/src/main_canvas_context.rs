// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry canvas context messages with their current native navigation and memory custody.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    canvas_context::{CanvasContextError, CanvasContextRequest, CanvasDisplayCapability},
    execution::{MemoryClaim, MemoryLease},
    panel::{PanelError, PanelSession},
    startup::MainIdentity,
};

/// A bounded public context request attributed to the current native top document.
pub struct CanvasContextObservation {
    identity: MainIdentity,
    request: CanvasContextRequest,
    lease: MemoryLease,
    claim: MemoryClaim,
}

/// An encoded context reply and its original native delivery allocation.
pub struct CanvasContextReply {
    identity: MainIdentity,
    payload: Vec<u8>,
    _claim: MemoryClaim,
}

/// Preserve navigation, allocation and shared-protocol failures.
#[derive(Debug)]
pub enum CanvasContextReplyError {
    /// The retained allocation does not cover bounded delivery.
    Storage,
    /// The request addresses another main navigation.
    ForeignNavigation,
    /// The current navigation cannot be represented as a panel session.
    Identity(PanelError),
    /// The context reply could not be encoded.
    Encode(CanvasContextError),
}

impl fmt::Display for CanvasContextReplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Storage => "Canvas context reply storage was not admitted",
            Self::ForeignNavigation => "Canvas context reply belongs to another navigation",
            Self::Identity(_) => "Canvas context reply navigation identity is invalid",
            Self::Encode(_) => "Canvas context reply could not be encoded",
        })
    }
}

impl Error for CanvasContextReplyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Identity(cause) => Some(cause),
            Self::Encode(cause) => Some(cause),
            Self::Storage | Self::ForeignNavigation => None,
        }
    }
}

impl CanvasContextObservation {
    /// Retain the attributed request and its original bounded parse allocation.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn new(
        identity: MainIdentity,
        request: CanvasContextRequest,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Self {
        Self { identity, request, lease, claim }
    }

    /// Borrow the native navigation that supplied this request.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Transfer the original request and admitted allocation to the application owner.
    pub fn into_parts(self) -> (MainIdentity, CanvasContextRequest, MemoryLease, MemoryClaim) {
        (self.identity, self.request, self.lease, self.claim)
    }
}

impl CanvasContextReply {
    /// Storage retained for the encoded reply and its native delivery serialization.
    pub const STORAGE_BYTES: u64 = 64 * 1024;

    /// Bind a display-only context to the exact original request and main navigation.
    ///
    /// # Errors
    /// Rejects inadequate storage, a foreign session, or an invalid encoded reply.
    pub fn new(
        identity: MainIdentity,
        request: &CanvasContextRequest,
        capability: CanvasDisplayCapability,
        claim: MemoryClaim,
    ) -> Result<Self, CanvasContextReplyError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(CanvasContextReplyError::Storage);
        }
        if request.session()
            != &PanelSession::for_main(identity).map_err(CanvasContextReplyError::Identity)?
        {
            return Err(CanvasContextReplyError::ForeignNavigation);
        }
        let payload = request.reply(capability).map_err(CanvasContextReplyError::Encode)?;
        Ok(Self { identity, payload, _claim: claim })
    }

    /// Borrow the exact native navigation addressed by this reply.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Borrow schema-validated UTF-8 while the original allocation remains retained.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}
