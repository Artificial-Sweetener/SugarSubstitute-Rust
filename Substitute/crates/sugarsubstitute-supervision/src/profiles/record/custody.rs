// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the exact prospective profile identity and independent witness binding.
use super::*;
use sugarsubstitute_contracts::backend_custody::{BackendCustodyKey, ProfileCustodyKey};
#[cfg(test)]
mod tests;

/// Schema-two extends the fixed160-byte row with genuine witness and family identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CustodiedRecord {
    pub(crate) original: Record,
    pub(crate) custodian: u64,
    pub(crate) registry_process: u64,
}
impl CustodiedRecord {
    /// Match the canonical persisted account against the helper's actual retained native SID.
    pub(crate) fn matches_account(self, sid: &[u8]) -> bool {
        self.original.account == super::account_identity(sid)
    }
    /// Check the real prepared source without exposing legacy record fields.
    pub(crate) fn matches_preparation(
        self,
        parent: BackendCustodyKey,
        key: ProfileCustodyKey,
    ) -> bool {
        self.original.run == key.run()
            && self.original.run.owner == parent.owner()
            && self.custodian == parent.session().get()
            && self.registry_process == key.registry_process()
            && self.original.state == State::Held
    }
    /// Observe only the profile directory identity retained by the canonical row.
    pub(crate) fn folder(self) -> [u8; 24] {
        self.original.folder
    }
    /// Derive prospective bytes only from the authentic same-owner custody source.
    pub(in crate::profiles) fn new(
        original: Record,
        parent: BackendCustodyKey,
        key: ProfileCustodyKey,
    ) -> Result<Self, ClaimError> {
        if original.run != key.run()
            || original.run.owner != parent.owner()
            || original.state != State::Held
        {
            return Err(ClaimError::invalid("profile_custody_record_association"));
        }
        Ok(Self {
            original,
            custodian: parent.session().get(),
            registry_process: key.registry_process(),
        })
    }
    /// Encode the fixed canonical row without changing the account/store/run binding.
    pub(crate) fn encode(self) -> [u8; RECORD_BYTES] {
        let mut bytes = self.original.encode();
        bytes[8..10].copy_from_slice(&2u16.to_le_bytes());
        bytes[136..144].copy_from_slice(&self.custodian.to_le_bytes());
        bytes[144..152].copy_from_slice(&self.registry_process.to_le_bytes());
        bytes
    }
    /// Decode canonical v2 bytes; legacyHeld never gains witness authority.
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ClaimError> {
        if bytes.len() != RECORD_BYTES
            || bytes[8..10] != [2, 0]
            || bytes[152..].iter().any(|byte| *byte != 0)
        {
            return Err(ClaimError::invalid("profile_custody_record_schema"));
        }
        let mut legacy: [u8; RECORD_BYTES] = bytes
            .try_into()
            .map_err(|cause| ClaimError::new("profile_custody_record_size", cause))?;
        legacy[8..10].copy_from_slice(&1u16.to_le_bytes());
        legacy[136..].fill(0);
        let original = Record::decode(&legacy)?;
        let number = |start| u64::from_le_bytes(std::array::from_fn(|index| bytes[start + index]));
        let value = Self { original, custodian: number(136), registry_process: number(144) };
        if value.custodian == 0 || value.registry_process == 0 || value.encode() != bytes {
            return Err(ClaimError::invalid("profile_custody_record_identity"));
        }
        Ok(value)
    }
    /// Produce Released only after the native owner proves the same physical family retired.
    pub(crate) fn released(self) -> Self {
        Self { original: Record { state: State::Released, ..self.original }, ..self }
    }
}
