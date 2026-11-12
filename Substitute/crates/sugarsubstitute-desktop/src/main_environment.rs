// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded native folder observations without granting profile authority.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Own exact observed UTF-16 data and its actual allocation claim until validation consumes it.
///
/// This observation cannot establish a canonical file identity or authorize a profile lease.
/// The root-owned resource/execution adapter validates those facts for the current main identity.
/// Storage is immutable and cannot be extracted independently of its admission guard.
pub struct MainEnvironmentFacts {
    units: Vec<u16>,
    claim: MemoryClaim,
}

impl MainEnvironmentFacts {
    /// Maximum retained non-NUL UTF-16 code units, excluding the native terminator.
    pub const MAX_UNITS: usize = 32766;
    /// Conservative known buffer/metadata claim required before a native getter/copy.
    /// Opaque browser/COM getter allocations remain separately qualified.
    pub const MAX_STORAGE_BYTES: u64 =
        (Self::MAX_UNITS * size_of::<u16>() + size_of::<Self>()) as u64;

    /// Adopt a previously bounded copy while retaining its real caller-owned claim.
    ///
    /// # Errors
    /// Rejects empty/NUL/excessive UTF-16 input, excessive actual Vec capacity, or insufficient
    /// claim bytes for that capacity plus fixed known metadata. This performs no native/FS work.
    pub fn new(units: Vec<u16>, claim: MemoryClaim) -> Result<Self, MainEnvironmentError> {
        let facts = Self { units, claim };
        if facts.units.is_empty() || facts.units.len() > Self::MAX_UNITS || facts.units.contains(&0)
        {
            return Err(MainEnvironmentError::InvalidFolder);
        }
        if facts.units.capacity() > Self::MAX_UNITS {
            return Err(MainEnvironmentError::ExcessCapacity);
        }
        let bytes = facts
            .units
            .capacity()
            .checked_mul(size_of::<u16>())
            .and_then(|bytes| bytes.checked_add(size_of::<Self>()))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(MainEnvironmentError::ExcessCapacity)?;
        if facts.claim.bytes() < bytes {
            return Err(MainEnvironmentError::InsufficientClaim);
        }
        Ok(facts)
    }
    /// Borrow exact observed UTF-16, including any unpaired surrogate, without native authority.
    pub fn as_utf16(&self) -> &[u16] {
        &self.units
    }
    /// Return held known-storage admission bytes, without reserving further capacity.
    pub fn claimed_bytes(&self) -> u64 {
        self.claim.bytes()
    }
}

impl fmt::Debug for MainEnvironmentFacts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MainEnvironmentFacts")
            .field("units", &self.units.len())
            .field("claimed_bytes", &self.claim.bytes())
            .finish_non_exhaustive()
    }
}

/// Reject malformed or unadmitted native observation storage without exposing its path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainEnvironmentError {
    /// Empty, NUL-containing or excessive UTF-16 data.
    InvalidFolder,
    /// Actual retained capacity exceeds the fixed known observation bound.
    ExcessCapacity,
    /// Supplied allocation claim cannot cover the actual copy and fixed metadata.
    InsufficientClaim,
}
impl fmt::Display for MainEnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Native environment observation is malformed or exceeds admitted storage")
    }
}
impl Error for MainEnvironmentError {}

#[cfg(test)]
#[path = "main_environment_tests.rs"]
mod tests;
