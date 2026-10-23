// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe the stable launcher's selection reader and the application entry it produces.
use super::{ApplicationContract, MAX_CONTRACT_BYTES, Rejection, valid_versions};
use serde::Deserialize;

/// Filename accompanying the producing launcher's authenticated executable payload.
pub const LAUNCHER_CONTRACT_FILE: &str = "sugarsubstitute-launcher.json";
/// Exact capabilities compiled into this stable launcher build.
pub const CURRENT_LAUNCHER_CONTRACT: &[u8] = include_bytes!("launcher.json");

/// A launcher uses one entry protocol and can read explicitly listed selection schemas.
/// Decoding establishes compatibility facts only; the package owner authenticates these bytes.
#[derive(Debug)]
pub struct LauncherContract {
    installation_schemas: Vec<u32>,
    installed_launch_protocol: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    installation_schemas: Vec<u32>,
    installed_launch_protocol: u32,
}
impl LauncherContract {
    /// Decode a bounded declaration without interpreting release numbers as capabilities.
    /// # Errors
    /// Rejects malformed, oversized, unknown, ambiguous or zero-valued declarations.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_CONTRACT_BYTES {
            return Err(Rejection::Format);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?;
        if wire.schema_version != 1
            || !valid_versions(&wire.installation_schemas)
            || wire.installed_launch_protocol == 0
        {
            return Err(Rejection::Format);
        }
        Ok(Self {
            installation_schemas: wire.installation_schemas,
            installed_launch_protocol: wire.installed_launch_protocol,
        })
    }

    /// Require both the installation's reader schema and the application's actual entry support.
    /// The caller supplies the selected installation schema, never one chosen by a remote offer.
    /// This check does not authorize publication, health promotion, migration or downgrade.
    /// # Errors
    /// Returns `InstallationSchema` for an unsupported selection reader or `Unsupported` when
    /// the application cannot receive the protocol this launcher produces.
    pub fn require_application(
        &self,
        application: &ApplicationContract,
        installation_schema: u32,
    ) -> Result<(), Rejection> {
        if !self.installation_schemas.contains(&installation_schema) {
            return Err(Rejection::InstallationSchema);
        }
        application.require_accepted_launch(self.installed_launch_protocol)
    }
}

#[cfg(test)]
mod tests;
