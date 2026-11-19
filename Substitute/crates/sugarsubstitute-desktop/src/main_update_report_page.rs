// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry whether the host has an update report to the current top document.

use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    host_update_report::{UpdateReportError, UpdateReportHostMessage},
    panel::PanelSession,
    startup::MainIdentity,
};

/// One host-direction update report message, encoded and retained with its admitted allocation.
pub(crate) struct UpdateReportCommand {
    identity: MainIdentity,
    payload: Vec<u8>,
    claim: MemoryClaim,
}

impl UpdateReportCommand {
    /// Storage for an encoded message of `length` bytes and its native command serialization,
    /// which may escape every byte once.
    const fn storage_bytes(length: usize) -> u64 {
        2 * length as u64 + 4096
    }

    /// Encode `message` for the exact current main session, funded from `lease` by its actual
    /// encoded size.
    ///
    /// # Errors
    /// Rejects an invalid native identity, a shared encoding failure or storage the lease
    /// cannot fund.
    pub(crate) fn new(
        identity: MainIdentity,
        message: &UpdateReportHostMessage,
        lease: &MemoryLease,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let session = PanelSession::for_main(identity).map_err(UpdateReportError::Session)?;
        let payload = message.encode(&session)?;
        let claim = lease.claim(Self::storage_bytes(payload.len()))?;
        Ok(Self { identity, payload, claim })
    }

    /// Borrow the addressed main-navigation identity.
    pub(crate) const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Borrow the encoded public message while its allocation claim remains retained.
    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Move oversized encoded bytes to the bulk owner; the claim remains with delivery custody.
    pub(crate) fn take_payload(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.payload)
    }

    /// Drop encoded storage before recovering its original noncloneable allocation claim.
    pub(crate) fn into_claim(self) -> MemoryClaim {
        drop(self.payload);
        self.claim
    }
}
