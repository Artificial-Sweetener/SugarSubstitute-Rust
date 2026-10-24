// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Declare durable installation activation separately from accepted-only launcher compatibility.
use super::{MAX_CONTRACT_BYTES, Rejection, valid_versions};
use serde::{Deserialize, Serialize};

/// Authenticated payload declaration emitted by the build producing the stable launcher.
pub const CONTRACT_FILE: &str = "sugarsubstitute-installation-activation.json";
/// Durable explicit-choice format implemented by the installation activation reader.
pub const AUTHORIZATION_SCHEMA: u32 = 1;

/// Actual launcher engine and explicit-choice reader; decoding grants no execution authority.
#[derive(Debug)]
pub struct InstallationActivationContract {
    authorization_schemas: Vec<u32>,
    engine_release: u64,
    engine_protocol: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    authorization_schemas: Vec<u32>,
    engine_release: String,
    engine_protocol: u32,
}

impl InstallationActivationContract {
    /// Emit this reader's capabilities with the producing binary's compiled engine identity.
    /// Decimal release text preserves all 64 bits in release tooling.
    /// # Errors
    /// Rejects zero identities and serialization failure; callers must supply their actual build.
    pub fn produced(engine_release: u64, engine_protocol: u32) -> Result<Vec<u8>, Rejection> {
        if engine_release == 0 || engine_protocol == 0 {
            return Err(Rejection::Format);
        }
        serde_json::to_vec(&Wire {
            schema_version: 1,
            authorization_schemas: vec![AUTHORIZATION_SCHEMA],
            engine_release: engine_release.to_string(),
            engine_protocol,
        })
        .map_err(|_| Rejection::Format)
    }

    /// Decode only bounded, canonical capability metadata from already authenticated bytes.
    /// # Errors
    /// Rejects malformed, unknown, duplicate or ambiguous fields and unsupported schema versions.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_CONTRACT_BYTES {
            return Err(Rejection::Format);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?;
        let release: u64 = wire.engine_release.parse().map_err(|_| Rejection::Format)?;
        if wire.schema_version != 1
            || !valid_versions(&wire.authorization_schemas)
            || release == 0
            || release.to_string() != wire.engine_release
            || wire.engine_protocol == 0
        {
            return Err(Rejection::Format);
        }
        Ok(Self {
            authorization_schemas: wire.authorization_schemas,
            engine_release: release,
            engine_protocol: wire.engine_protocol,
        })
    }

    /// Require a launcher that can independently reopen the committed installation choice.
    /// The minimum engine release comes from the authenticated package offer.
    /// # Errors
    /// Rejects absent reader support, a different engine protocol, or an older bundled engine.
    pub fn require_installation(
        &self,
        authorization_schema: u32,
        minimum_engine_release: u64,
        engine_protocol: u32,
    ) -> Result<(), Rejection> {
        if !self.authorization_schemas.contains(&authorization_schema)
            || minimum_engine_release == 0
            || self.engine_release < minimum_engine_release
            || self.engine_protocol != engine_protocol
        {
            return Err(Rejection::Unsupported);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
