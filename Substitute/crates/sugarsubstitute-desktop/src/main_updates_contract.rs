// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry the page's update requests and the host's answers between the composing owner and the
//! current top document.

use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_updates::{UpdatesClientMessage, UpdatesError, UpdatesHostMessage},
    panel::PanelSession,
    startup::MainIdentity,
};

/// A bounded page update request attributed to the current native document.
/// Attribution is not permission to change native state beyond what the updater allows.
pub struct UpdatesObservation {
    identity: MainIdentity,
    message: UpdatesClientMessage,
    lease: MemoryLease,
    claim: MemoryClaim,
}

impl UpdatesObservation {
    /// Retain the parse's actual admission alongside the attributed message.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn new(
        identity: MainIdentity,
        message: UpdatesClientMessage,
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
    pub fn into_parts(self) -> (MainIdentity, UpdatesClientMessage, MemoryLease, MemoryClaim) {
        (self.identity, self.message, self.lease, self.claim)
    }
}

/// One host-direction updates message, encoded and retained with its admitted allocation.
pub struct UpdatesCommand {
    identity: MainIdentity,
    revision: Option<u64>,
    payload: Vec<u8>,
    claim: MemoryClaim,
}

impl UpdatesCommand {
    /// Admitted storage for the encoded message and its native command serialization.
    pub const STORAGE_BYTES: u64 = 20480;

    /// Encode `message` for the exact current main session.
    ///
    /// # Errors
    /// Rejects inadequate storage, an invalid native identity or a shared encoding failure.
    pub fn new(
        identity: MainIdentity,
        message: &UpdatesHostMessage,
        claim: MemoryClaim,
    ) -> Result<Self, UpdatesError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(UpdatesError::Invalid);
        }
        let session = PanelSession::for_main(identity).map_err(UpdatesError::Session)?;
        let payload = message.encode(&session)?;
        let revision = match message {
            UpdatesHostMessage::State { revision, .. } => Some(*revision),
            UpdatesHostMessage::Accepted(_) | UpdatesHostMessage::Rejected { .. } => None,
        };
        Ok(Self { identity, revision, payload, claim })
    }

    /// Borrow the addressed main-navigation identity.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// The state's revision; a newer pending state replaces an older one. Answers have none and
    /// are never replaced.
    pub const fn revision(&self) -> Option<u64> {
        self.revision
    }

    /// Borrow the encoded public message while its allocation claim remains retained.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Move oversized encoded bytes to the bulk owner; the claim remains with delivery custody.
    #[cfg(target_os = "windows")]
    pub(crate) fn take_payload(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.payload)
    }

    /// Drop encoded storage before recovering its original noncloneable allocation claim.
    pub(crate) fn into_claim(self) -> MemoryClaim {
        drop(self.payload);
        self.claim
    }
}
