// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer an attributed panel message together with its admitted allocation custody.

use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    panel::{PanelError, PanelMessage, PanelSession},
    startup::MainIdentity,
};

/// A bounded public panel message attributed to the current native document.
/// Attribution is not permission to access files, networks or native windows.
pub struct PanelObservation {
    identity: MainIdentity,
    message: PanelMessage,
    lease: MemoryLease,
    claim: MemoryClaim,
}

/// A host-direction panel message retained with its admitted allocation.
pub struct PanelCommand {
    identity: MainIdentity,
    message: PanelMessage,
    _claim: MemoryClaim,
}

impl PanelCommand {
    /// Admitted storage for one owned message and its serialized command input.
    pub const STORAGE_BYTES: u64 = 4096;

    /// Bind a configuration or heartbeat to the same complete main-navigation identity.
    ///
    /// # Errors
    /// Rejects a client-direction or foreign-session message before any native effect.
    pub fn new(
        identity: MainIdentity,
        message: PanelMessage,
        claim: MemoryClaim,
    ) -> Result<Self, PanelError> {
        if claim.bytes() < Self::STORAGE_BYTES
            || message.session() != &PanelSession::for_main(identity)?
            || !matches!(message, PanelMessage::Configure { .. } | PanelMessage::Ping(_))
        {
            return Err(PanelError::Invalid);
        }
        Ok(Self { identity, message, _claim: claim })
    }

    /// Borrow the addressed main-navigation identity.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Borrow the bounded public message while its allocation claim remains retained.
    pub fn message(&self) -> &PanelMessage {
        &self.message
    }
}

impl PanelObservation {
    /// Retain the parse's actual admission alongside the attributed message.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn new(
        identity: MainIdentity,
        message: PanelMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Self {
        Self { identity, message, lease, claim }
    }

    /// Borrow the native document identity that supplied the observation.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Transfer the message and its original admission to the composing application owner.
    pub fn into_parts(self) -> (MainIdentity, PanelMessage, MemoryLease, MemoryClaim) {
        (self.identity, self.message, self.lease, self.claim)
    }
}
