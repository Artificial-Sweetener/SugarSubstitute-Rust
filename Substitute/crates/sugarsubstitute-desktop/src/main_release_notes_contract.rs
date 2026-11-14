// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry the page's release notes requests and the host's answers between the composing owner
//! and the current top document.

use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_release_notes::{ReleaseNotesClientMessage, ReleaseNotesError, ReleaseNotesHostMessage},
    panel::PanelSession,
    startup::MainIdentity,
};

/// A bounded page release notes request attributed to the current native document.
/// Attribution grants nothing beyond reading the host's published notes.
pub struct ReleaseNotesObservation {
    identity: MainIdentity,
    message: ReleaseNotesClientMessage,
    lease: MemoryLease,
    claim: MemoryClaim,
}

impl ReleaseNotesObservation {
    /// Retain the parse's actual admission alongside the attributed message.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn new(
        identity: MainIdentity,
        message: ReleaseNotesClientMessage,
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
    pub fn into_parts(self) -> (MainIdentity, ReleaseNotesClientMessage, MemoryLease, MemoryClaim) {
        (self.identity, self.message, self.lease, self.claim)
    }
}

/// One host-direction release notes message, encoded and retained with its admitted allocation.
pub struct ReleaseNotesCommand {
    identity: MainIdentity,
    revision: Option<u64>,
    payload: Vec<u8>,
    claim: MemoryClaim,
}

impl ReleaseNotesCommand {
    /// Storage for an encoded message of `length` bytes and its native command serialization,
    /// which may escape every byte once.
    pub const fn storage_bytes(length: usize) -> u64 {
        2 * length as u64 + 4096
    }

    /// Encode `message` for the exact current main session, funded from `lease` by its actual
    /// encoded size, so a state costs a short message and notes cost what they carry.
    ///
    /// # Errors
    /// Rejects an invalid native identity, a shared encoding failure or storage the lease
    /// cannot fund.
    pub fn new(
        identity: MainIdentity,
        message: &ReleaseNotesHostMessage,
        lease: &MemoryLease,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let session = PanelSession::for_main(identity).map_err(ReleaseNotesError::Session)?;
        let payload = message.encode(&session)?;
        let claim = lease.claim(Self::storage_bytes(payload.len()))?;
        let revision = match message {
            ReleaseNotesHostMessage::State { revision, .. } => Some(*revision),
            ReleaseNotesHostMessage::Open
            | ReleaseNotesHostMessage::Notes { .. }
            | ReleaseNotesHostMessage::Failed { .. } => None,
        };
        Ok(Self { identity, revision, payload, claim })
    }

    /// Borrow the addressed main-navigation identity.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// The state's revision; a newer pending state replaces an older one. Other messages have
    /// none and are never replaced.
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
