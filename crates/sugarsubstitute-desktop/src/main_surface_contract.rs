// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry surface messages with their current native identity and allocation custody.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    canvas_surface::{
        CanvasSurfaceCapabilities, CanvasSurfaceDiscoveryRequest, CanvasSurfaceDiscoveryStatus,
        CanvasSurfaceMessage, CanvasSurfaceMessageError, CanvasSurfaceReplyStatus,
        CanvasSurfaceRequest, CanvasViewportState,
    },
    execution::{MemoryClaim, MemoryLease},
    panel::{PanelError, PanelSession},
    startup::MainIdentity,
};

/// A bounded untrusted canvas message from the current top document.
pub struct SurfaceObservation {
    identity: MainIdentity,
    message: CanvasSurfaceMessage,
    lease: MemoryLease,
    claim: MemoryClaim,
    #[cfg(feature = "native-composition")]
    allocation: Option<crate::canvas::placement::WebViewAllocation>,
}

/// An encoded surface reply retaining the allocation needed by native delivery.
pub struct SurfaceReply {
    identity: MainIdentity,
    #[cfg(any(target_os = "windows", test))]
    request_id: u64,
    payload: Vec<u8>,
    _claim: MemoryClaim,
}

/// Preserve admission, navigation and shared-protocol failures at the native boundary.
#[derive(Debug)]
pub enum SurfaceReplyError {
    /// The retained allocation does not cover native delivery.
    Storage,
    /// The request is correlated to a different main navigation.
    ForeignNavigation,
    /// The current navigation could not be represented as a panel session.
    Identity(PanelError),
    /// The shared reply could not be encoded.
    Encode(CanvasSurfaceMessageError),
}

impl fmt::Display for SurfaceReplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Storage => "Surface reply storage was not admitted",
            Self::ForeignNavigation => "Surface reply belongs to a different navigation",
            Self::Identity(_) => "Surface reply navigation identity is invalid",
            Self::Encode(_) => "Surface reply could not be encoded",
        })
    }
}

impl Error for SurfaceReplyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Identity(cause) => Some(cause),
            Self::Encode(cause) => Some(cause),
            Self::Storage | Self::ForeignNavigation => None,
        }
    }
}

impl SurfaceObservation {
    /// Retain the native ingress claim alongside the decoded message.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn new(
        identity: MainIdentity,
        message: CanvasSurfaceMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Self {
        Self {
            identity,
            message,
            lease,
            claim,
            #[cfg(feature = "native-composition")]
            allocation: None,
        }
    }

    /// Borrow the identity that supplied this observation.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Borrow the untrusted operation before projecting its geometry against the native view.
    #[cfg(feature = "native-composition")]
    pub(crate) fn message(&self) -> &CanvasSurfaceMessage {
        &self.message
    }

    /// Attach only the creating apartment's current WebView bounds and scale.
    #[cfg(feature = "native-composition")]
    pub(crate) fn set_allocation(
        &mut self,
        allocation: crate::canvas::placement::WebViewAllocation,
    ) {
        self.allocation = Some(allocation);
    }

    /// Return the native allocation observed with this operation, when available.
    #[cfg(feature = "native-composition")]
    pub const fn allocation(&self) -> Option<crate::canvas::placement::WebViewAllocation> {
        self.allocation
    }

    /// Transfer the actual input and its original admitted allocation.
    pub fn into_parts(self) -> (MainIdentity, CanvasSurfaceMessage, MemoryLease, MemoryClaim) {
        (self.identity, self.message, self.lease, self.claim)
    }
}

impl SurfaceReply {
    /// Space retained for the encoded response and its native delivery serialization.
    pub const STORAGE_BYTES: u64 = 64 * 1024;

    /// Encode a correlated response for the exact current main navigation.
    ///
    /// # Errors
    /// Rejects inadequate storage, a foreign session, invalid status, or encoding failure.
    pub fn new(
        identity: MainIdentity,
        request: &CanvasSurfaceRequest,
        status: CanvasSurfaceReplyStatus,
        message: &str,
        claim: MemoryClaim,
    ) -> Result<Self, SurfaceReplyError> {
        Self::validate_custody(identity, request.session(), &claim)?;
        let payload = request.reply(status, message).map_err(SurfaceReplyError::Encode)?;
        Ok(Self {
            identity,
            #[cfg(any(target_os = "windows", test))]
            request_id: request.request_id(),
            payload,
            _claim: claim,
        })
    }

    /// Encode declared surface support through the same navigation-qualified delivery custody.
    /// Discovery describes implemented operations; it grants no attachment or backend authority.
    ///
    /// # Errors
    /// Rejects inadequate storage, a foreign navigation or an invalid capability response.
    pub fn discovery(
        identity: MainIdentity,
        request: &CanvasSurfaceDiscoveryRequest,
        status: CanvasSurfaceDiscoveryStatus,
        capabilities: &CanvasSurfaceCapabilities,
        message: &str,
        claim: MemoryClaim,
    ) -> Result<Self, SurfaceReplyError> {
        Self::validate_custody(identity, request.session(), &claim)?;
        let payload =
            request.reply(status, capabilities, message).map_err(SurfaceReplyError::Encode)?;
        Ok(Self {
            identity,
            #[cfg(any(target_os = "windows", test))]
            request_id: request.request_id(),
            payload,
            _claim: claim,
        })
    }

    /// Encode the native owner's observed viewport with the original delivery allocation.
    ///
    /// # Errors
    /// Rejects inadequate storage, foreign navigation, or an invalid viewport response.
    pub fn viewport(
        identity: MainIdentity,
        request: &CanvasSurfaceRequest,
        viewport: &CanvasViewportState,
        message: &str,
        claim: MemoryClaim,
    ) -> Result<Self, SurfaceReplyError> {
        Self::validate_custody(identity, request.session(), &claim)?;
        let payload =
            request.viewport_reply(viewport, message).map_err(SurfaceReplyError::Encode)?;
        Ok(Self {
            identity,
            #[cfg(any(target_os = "windows", test))]
            request_id: request.request_id(),
            payload,
            _claim: claim,
        })
    }

    /// Apply identical native identity and retained-storage checks to every surface response.
    fn validate_custody(
        identity: MainIdentity,
        session: &PanelSession,
        claim: &MemoryClaim,
    ) -> Result<(), SurfaceReplyError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(SurfaceReplyError::Storage);
        }
        if session != &PanelSession::for_main(identity).map_err(SurfaceReplyError::Identity)? {
            return Err(SurfaceReplyError::ForeignNavigation);
        }
        Ok(())
    }

    /// Borrow the native navigation addressed by this reply.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Borrow the schema-validated UTF-8 payload while its admission remains held.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Match delivery to the admitted ordinary request without decoding its response again.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Return the original credit only after the native delivery owner observes completion.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn into_claim(self) -> MemoryClaim {
        let Self { payload, _claim, .. } = self;
        drop(payload);
        _claim
    }
}

#[cfg(test)]
mod tests;
