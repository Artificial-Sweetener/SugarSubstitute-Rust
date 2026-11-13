// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer an attributed panel message together with its admitted allocation custody.

use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host::HostDescription,
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
    hosting: crate::MainHosting,
}

/// A host-direction panel message retained with its admitted allocation.
pub struct PanelCommand {
    identity: MainIdentity,
    description: Option<HostDescription>,
    message: PanelMessage,
    _claim: MemoryClaim,
}

impl PanelCommand {
    /// Admitted storage for one owned message and its serialized command input.
    pub const STORAGE_BYTES: u64 = 4096;

    /// Bind a configuration or heartbeat to the same complete main-navigation identity.
    ///
    /// A configuration carries the host `description` the page receives immediately before it,
    /// in the same native delivery; a heartbeat carries none.
    ///
    /// # Errors
    /// Rejects a client-direction or foreign-session message, a configuration without its
    /// description and a heartbeat with one, before any native effect.
    pub fn new(
        identity: MainIdentity,
        description: Option<HostDescription>,
        message: PanelMessage,
        claim: MemoryClaim,
    ) -> Result<Self, PanelError> {
        let paired = match message {
            PanelMessage::Configure { .. } => description.is_some(),
            PanelMessage::Ping(_) => description.is_none(),
            _ => false,
        };
        if claim.bytes() < Self::STORAGE_BYTES
            || message.session() != &PanelSession::for_main(identity)?
            || !paired
        {
            return Err(PanelError::Invalid);
        }
        Ok(Self { identity, description, message, _claim: claim })
    }

    /// Borrow the host description delivered before a configuration.
    pub fn description(&self) -> Option<&HostDescription> {
        self.description.as_ref()
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
        hosting: crate::MainHosting,
    ) -> Self {
        Self { identity, message, lease, claim, hosting }
    }

    /// How the main browser that sent this message is hosted, which decides what the host
    /// declares to it.
    pub const fn hosting(&self) -> crate::MainHosting {
        self.hosting
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
