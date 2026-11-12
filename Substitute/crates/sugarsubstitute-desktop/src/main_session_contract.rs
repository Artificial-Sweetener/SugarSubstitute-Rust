// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer independently bootstrapped workspace messages with their admitted storage.
#[cfg(windows)]
use crate::DesktopError;
use crate::MainIdentity;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    desktop_session::{
        MAX_MESSAGE_BYTES, SessionError, validate_message,
        wire::{SessionCapabilities, SessionFence, SessionMessage, SessionOperation, SessionScope},
    },
    execution::{MemoryClaim, MemoryLease},
};
#[cfg(any(windows, test))]
pub(crate) const MAX_INGRESS_BYTES: usize = MAX_MESSAGE_BYTES + 1024;

/// Trusted host-selected workspace bootstrap for one actual native main lifetime.
/// Construction carries matching data only; backend/profile admission remains independent.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionBootstrap {
    scope: SessionScope,
    origin: LoopbackOrigin,
    webview: String,
    media: bool,
}
impl SessionBootstrap {
    /// Validate the host's selected scope, origin and fresh canonical webview identity.
    /// The caller obtains these from native installation/profile/backend owners, never a page.
    /// # Errors
    /// Refuses malformed namespace or webview identities and excessive retained capacities.
    pub fn new(
        scope: SessionScope,
        origin: LoopbackOrigin,
        webview: String,
    ) -> Result<Self, SessionError> {
        let message = SessionMessage {
            protocol: "sugarcubes.desktop-session".into(),
            version: 1,
            request_id: webview.clone(),
            scope: scope.clone(),
            origin: origin_text(origin),
            operation: SessionOperation::Ready {
                capabilities: SessionCapabilities {
                    capture: false,
                    materialize: false,
                    chunk_transfer: false,
                    artifact_resolution: false,
                },
                committed: false,
                workflow_persistence: true,
                fence: SessionFence { webview: webview.clone(), revision: 0, bindings: vec![] },
            },
        };
        validate_message(&message, &scope, &message.origin)?;
        let result = Self { scope, origin, webview, media: false };
        if result.allocated_bytes() > 4096 {
            return Err(SessionError::TooLarge);
        }
        Ok(result)
    }
    /// Enable the version-one retained-media capability; actual URLs are allocated natively.
    pub fn with_media(mut self, enabled: bool) -> Self {
        self.media = enabled;
        self
    }
    /// Borrow the independently selected durable namespace.
    pub fn scope(&self) -> &SessionScope {
        &self.scope
    }
    /// Return the independently admitted loopback origin.
    pub const fn origin(&self) -> LoopbackOrigin {
        self.origin
    }
    /// Serialize the admitted browser origin through the desktop's canonical navigation owner.
    /// Browser default-port omission matches `window.location.origin` and native message checks.
    pub fn origin_text(&self) -> String {
        origin_text(self.origin)
    }
    /// Borrow this native browser lifetime's host-granted UUID.
    pub fn webview(&self) -> &str {
        &self.webview
    }
    /// Whether this lifetime may advertise the optional version-one media capability.
    pub const fn media_enabled(&self) -> bool {
        self.media
    }
    /// Actual owned string capacities retained by the inert bootstrap, in bytes.
    pub(crate) fn allocated_bytes(&self) -> usize {
        self.scope.installation.capacity()
            + self.scope.profile.capacity()
            + self.scope.workspace.capacity()
            + self.scope.backend.capacity()
            + self.webview.capacity()
    }
    /// Validate message scope/origin and its webview fence without admitting application effects.
    #[cfg(any(windows, test))]
    pub(crate) fn validate(&self, message: &SessionMessage) -> Result<(), SessionError> {
        validate_message(message, &self.scope, &origin_text(self.origin))?;
        let fence = match &message.operation {
            SessionOperation::Ready { fence, .. }
            | SessionOperation::Capture { fence, .. }
            | SessionOperation::Materialize { fence, .. } => Some(fence),
            _ => None,
        };
        if fence.is_some_and(|fence| fence.webview != self.webview) {
            return Err(SessionError::Stale);
        }
        Ok(())
    }
}

/// Bounded session input attributed to an actual native top document and storage lease.
pub struct SessionObservation {
    identity: MainIdentity,
    message: SessionMessage,
    lease: MemoryLease,
    claim: MemoryClaim,
}
impl SessionObservation {
    #[cfg(any(windows, test))]
    pub(crate) fn new(
        identity: MainIdentity,
        message: SessionMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Self {
        Self { identity, message, lease, claim }
    }
    /// Return the actual native main lifetime that observed this message.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }
    /// Borrow a validated message while its original admission remains retained.
    pub fn message(&self) -> &SessionMessage {
        &self.message
    }
    /// Transfer decoded input and its original native admission to the application owner.
    pub fn into_parts(self) -> (MainIdentity, SessionMessage, MemoryLease, MemoryClaim) {
        (self.identity, self.message, self.lease, self.claim)
    }
}

/// One host-to-page session operation tied to the same actual main lifetime.
pub struct SessionCommand {
    identity: MainIdentity,
    message: SessionMessage,
    _claim: MemoryClaim,
}
impl SessionCommand {
    /// Maximum DTO/UTF-8/UTF-16 staging admission for one native message.
    pub const STORAGE_BYTES: u64 = (MAX_MESSAGE_BYTES as u64) * 4 + 4096;
    /// Admit one host-directed bounded message under independently trusted scope and origin.
    /// Native delivery rechecks it against the current bootstrap before posting.
    /// # Errors
    /// Refuses insufficient storage, malformed input, foreign origin or namespace.
    pub fn new(
        identity: MainIdentity,
        message: SessionMessage,
        claim: MemoryClaim,
        scope: &SessionScope,
        origin: &str,
    ) -> Result<Self, SessionError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(SessionError::TooLarge);
        }
        validate_message(&message, scope, origin)?;
        Ok(Self { identity, message, _claim: claim })
    }
    /// Borrow the addressed native main lifetime.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }
    /// Borrow the bounded operation with its storage charge retained.
    pub fn message(&self) -> &SessionMessage {
        &self.message
    }
}

/// Exact existing loopback host/port spelling, excluding the root URL's trailing slash.
pub(crate) fn origin_text(origin: LoopbackOrigin) -> String {
    let root = crate::main_navigation::root_url(origin);
    if origin.port().get() == 80 {
        root.trim_end_matches(":80/").to_owned()
    } else {
        root.trim_end_matches('/').to_owned()
    }
}

/// Retain a contract failure at the native adapter boundary without stringly typed cause loss.
#[cfg(windows)]
pub(crate) fn contract_error(operation: &'static str, cause: SessionError) -> DesktopError {
    DesktopError::new(operation, cause)
}
#[cfg(test)]
mod tests;
