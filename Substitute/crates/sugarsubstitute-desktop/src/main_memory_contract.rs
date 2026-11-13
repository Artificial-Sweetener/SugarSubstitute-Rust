// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry the memory budget between the composing owner and the current top document.

use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    memory::{MemoryAllowance, MemoryError, MemoryMessage},
    panel::PanelSession,
    startup::MainIdentity,
};

/// A bounded page memory report attributed to the current native document.
/// Attribution is not permission to change native state beyond the memory budget.
pub struct MemoryObservation {
    identity: MainIdentity,
    message: MemoryMessage,
    lease: MemoryLease,
    claim: MemoryClaim,
}

impl MemoryObservation {
    /// Retain the parse's actual admission alongside the attributed message.
    #[cfg(target_os = "windows")]
    pub(crate) fn new(
        identity: MainIdentity,
        message: MemoryMessage,
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
    pub fn into_parts(self) -> (MainIdentity, MemoryMessage, MemoryLease, MemoryClaim) {
        (self.identity, self.message, self.lease, self.claim)
    }
}

/// One host-direction memory allowance, encoded and retained with its admitted allocation.
pub struct MemoryCommand {
    identity: MainIdentity,
    #[cfg(target_os = "windows")]
    generation: u64,
    #[cfg(target_os = "windows")]
    payload: Vec<u8>,
    #[cfg(target_os = "windows")]
    claim: MemoryClaim,
    /// Only the Windows host delivers the command; elsewhere it keeps its admission until drop.
    #[cfg(not(target_os = "windows"))]
    _claim: MemoryClaim,
}

impl MemoryCommand {
    /// Admitted storage for the encoded allowance and its native command serialization.
    pub const STORAGE_BYTES: u64 = 4096;

    /// Encode `allowance` for the exact current main session.
    ///
    /// # Errors
    /// Rejects inadequate storage, an invalid native identity or a shared encoding failure.
    pub fn new(
        identity: MainIdentity,
        allowance: &MemoryAllowance,
        claim: MemoryClaim,
    ) -> Result<Self, MemoryError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(MemoryError::Invalid);
        }
        let session = PanelSession::for_main(identity).map_err(MemoryError::Session)?;
        let payload = allowance.encode(&session)?;
        #[cfg(target_os = "windows")]
        let command = Self { identity, generation: allowance.generation, payload, claim };
        #[cfg(not(target_os = "windows"))]
        let command = {
            drop(payload);
            Self { identity, _claim: claim }
        };
        Ok(command)
    }

    /// Borrow the addressed main-navigation identity.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    #[cfg(target_os = "windows")]
    /// The allowance generation; a newer pending allowance replaces an older one.
    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg(target_os = "windows")]
    /// Borrow the encoded public message while its allocation claim remains retained.
    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Move oversized encoded bytes to the bulk owner; the claim remains with delivery custody.
    #[cfg(target_os = "windows")]
    pub(crate) fn take_payload(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.payload)
    }

    #[cfg(target_os = "windows")]
    /// Drop encoded storage before recovering its original noncloneable allocation claim.
    pub(crate) fn into_claim(self) -> MemoryClaim {
        drop(self.payload);
        self.claim
    }
}
