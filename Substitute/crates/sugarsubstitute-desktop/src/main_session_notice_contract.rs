// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry the page's session notice messages and the host's notices between the composing owner
//! and the current top document.

use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_session_notice::{
        SessionNoticeClientMessage, SessionNoticeError, SessionNoticeHostMessage,
    },
    panel::PanelSession,
    startup::MainIdentity,
};

/// A bounded page session notice message attributed to the current native document.
/// Attribution grants nothing beyond learning and settling the notice the host owes.
pub struct SessionNoticeObservation {
    identity: MainIdentity,
    message: SessionNoticeClientMessage,
    lease: MemoryLease,
    claim: MemoryClaim,
}

impl SessionNoticeObservation {
    /// Retain the parse's actual admission alongside the attributed message.
    #[cfg(any(target_os = "windows", test))]
    #[cfg(target_os = "windows")]
    pub(crate) fn new(
        identity: MainIdentity,
        message: SessionNoticeClientMessage,
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
    pub fn into_parts(
        self,
    ) -> (MainIdentity, SessionNoticeClientMessage, MemoryLease, MemoryClaim) {
        (self.identity, self.message, self.lease, self.claim)
    }
}

/// One host-direction session notice message, encoded and retained with its admitted allocation.
pub struct SessionNoticeCommand {
    identity: MainIdentity,
    payload: Vec<u8>,
    #[cfg(target_os = "windows")]
    claim: MemoryClaim,
    /// Only the Windows host delivers the command; elsewhere it keeps its admission until drop.
    #[cfg(not(target_os = "windows"))]
    _claim: MemoryClaim,
}

impl SessionNoticeCommand {
    /// Storage for an encoded message of `length` bytes and its native command serialization,
    /// which may escape every byte once.
    pub const fn storage_bytes(length: usize) -> u64 {
        2 * length as u64 + 4096
    }

    /// Encode `message` for the exact current main session, funded from `lease` by its actual
    /// encoded size.
    ///
    /// # Errors
    /// Rejects an invalid native identity, a shared encoding failure or storage the lease
    /// cannot fund.
    pub fn new(
        identity: MainIdentity,
        message: &SessionNoticeHostMessage,
        lease: &MemoryLease,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let session = PanelSession::for_main(identity).map_err(SessionNoticeError::Session)?;
        let payload = message.encode(&session)?;
        let claim = lease.claim(Self::storage_bytes(payload.len()))?;
        Ok(Self {
            identity,
            payload,
            #[cfg(target_os = "windows")]
            claim,
            #[cfg(not(target_os = "windows"))]
            _claim: claim,
        })
    }

    /// Borrow the addressed main-navigation identity.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
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
    #[cfg(target_os = "windows")]
    pub(crate) fn into_claim(self) -> MemoryClaim {
        drop(self.payload);
        self.claim
    }
}
