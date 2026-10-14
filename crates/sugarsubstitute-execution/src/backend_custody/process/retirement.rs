// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one close-only cleanup cutoff for the original finite creation.
use super::CustodiedProcessMemory;
use sugarsubstitute_contracts::execution::{AdmissionError, Moment};

/// Original close-only proof; cancellation does not grant another creation or ACK.
/// Every alias retains the original fixed family claim and first15-second cutoff.
#[derive(Clone)]
pub struct CustodiedProcessRetirement {
    pub(super) memory: CustodiedProcessMemory,
    pub(super) until: Moment,
}
impl CustodiedProcessRetirement {
    /// Observe the first immutable cleanup cutoff in the original execution epoch.
    pub const fn until(&self) -> Moment {
        self.until
    }
    /// Match the actual original allocation without providers or caller IDs.
    pub fn matches(&self, memory: &CustodiedProcessMemory) -> bool {
        self.memory.same_allocation(memory)
    }
    /// Check the actual running original owner before closing an already-owned output.
    /// # Errors
    /// Preserves original provider failure, changed owner or first-cutoff expiry.
    pub fn check(&self) -> Result<Moment, AdmissionError> {
        let now = self.memory.observe_retirement()?;
        if now >= self.until {
            return Err(AdmissionError::DeadlineExpired { deadline: self.until, now });
        }
        Ok(now)
    }
}
