// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded correlation carried by a native updater probe; it supplies no repository authority.
use super::Rejection;
use crate::installation::Trial;

/// Native probe verifies the exact pending attempt and exits after its execution owner retires.
/// Version 3 carries a trial whose generation names its signed release; version 2 did not.
pub const PROBE_PROTOCOL: u32 = 3;
/// Follow with an installation path, encoded `ProbeRequest` and inherited exclusive disk handle.
/// The child captures the handle once at initial entry and verifies its native identity before use.
pub const PROBE_ARGUMENT: &str = "--internal-updater-probe-v3";
/// Encoded correlation bound in UTF-8 bytes, checked before decoding untrusted JSON.
pub const MAX_PROBE_BYTES: usize = 1024;

/// Correlation only: successful decoding cannot prove process identity, health or package trust.
pub struct ProbeRequest {
    attempt: Trial,
}
impl ProbeRequest {
    /// Construct an explicit positive-revision attempt; new installations have no probe authority.
    /// # Errors
    /// Rejects a zero revision, which cannot identify an admitted trial.
    pub fn new(attempt: Trial) -> Result<Self, Rejection> {
        if attempt.revision == 0 {
            return Err(Rejection::Format);
        }
        Ok(Self { attempt })
    }
    /// Decode bounded correlation while preserving strict installation/generation identities.
    /// # Errors
    /// Rejects malformed, oversized, duplicate/unknown fields or a zero revision.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_PROBE_BYTES {
            return Err(Rejection::Format);
        }
        Self::new(serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?)
    }
    /// Encode one argument for the native command composer; shell quoting is a separate concern.
    /// # Errors
    /// Rejects serialization failure or output exceeding the protocol bound.
    pub fn encode(&self) -> Result<String, Rejection> {
        let text = serde_json::to_string(&self.attempt).map_err(|_| Rejection::Format)?;
        if text.len() > MAX_PROBE_BYTES {
            return Err(Rejection::Format);
        }
        Ok(text)
    }
    /// Exact requested trial; the receiver matches it to protected current state.
    pub fn attempt(&self) -> &Trial {
        &self.attempt
    }
}

#[cfg(test)]
mod tests;
