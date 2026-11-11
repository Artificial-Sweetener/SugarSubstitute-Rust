// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind one inherited transport endpoint to its creator's unpredictable request identity.
use super::Rejection;
use serde::{Deserialize, Serialize};

/// Bound the encoded command argument before decoding or capturing native custody.
pub const MAX_REQUEST_BYTES: usize = 1024;

/// Correlation and a native slot only; neither grants package or installation authority.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationRequest {
    channel: u64,
    identity: [u8; 32],
}
impl ActivationRequest {
    /// Construct a nonzero identity with a positive, non-pseudo native handle value.
    /// # Errors
    /// Rejects absent correlation and slots outside the portable signed native range.
    pub fn new(channel: u64, identity: [u8; 32]) -> Result<Self, Rejection> {
        if channel == 0 || channel > i64::MAX as u64 || identity == [0; 32] {
            return Err(Rejection::Format);
        }
        Ok(Self { channel, identity })
    }
    /// Decode strict bounded input before any platform-specific slot adoption.
    /// # Errors
    /// Rejects oversized, malformed, duplicate, unknown or invalid fields.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(Rejection::Format);
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?;
        Self::new(value.channel, value.identity)
    }
    /// Encode the argument consumed by the native command composer, without shell interpretation.
    /// # Errors
    /// Preserves encoding or protocol-bound failure.
    pub fn encode(&self) -> Result<String, Rejection> {
        let text = serde_json::to_string(self).map_err(|_| Rejection::Format)?;
        if text.len() > MAX_REQUEST_BYTES {
            return Err(Rejection::Format);
        }
        Ok(text)
    }
    /// Native handle number, valid only in the explicitly inheriting child's initial entry.
    pub fn channel(&self) -> u64 {
        self.channel
    }
    /// Bind request and reply bytes to the creator that supplied this endpoint.
    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_correlation_roundtrips_without_granting_unbounded_native_slots()
    -> Result<(), Rejection> {
        let request = ActivationRequest::new(12, [19; 32])?;
        let decoded = ActivationRequest::decode(request.encode()?.as_bytes())?;
        assert_eq!(decoded.channel(), 12);
        assert_eq!(decoded.identity(), [19; 32]);
        for slot in [0, u64::MAX, i64::MAX as u64 + 1] {
            assert!(ActivationRequest::new(slot, [19; 32]).is_err());
        }
        assert!(ActivationRequest::new(12, [0; 32]).is_err());
        for text in [
            "{}".to_owned(),
            " ".repeat(MAX_REQUEST_BYTES + 1),
            request.encode()?.replace("\"channel\":12", "\"channel\":12,\"channel\":13"),
            request.encode()?.replace("\"channel\":12", "\"channel\":12,\"authority\":true"),
        ] {
            assert!(ActivationRequest::decode(text.as_bytes()).is_err());
        }
        Ok(())
    }
}
