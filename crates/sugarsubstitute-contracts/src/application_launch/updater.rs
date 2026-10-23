// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Declare the updater's entry and installation reader independently of desktop entry.
use super::{MAX_CONTRACT_BYTES, Rejection, valid_versions};
use serde::Deserialize;
pub mod probe;

/// Authenticated declaration accompanying the independently executable updater.
pub const UPDATER_CONTRACT_FILE: &str = "sugarsubstitute-updater.json";
/// Exact entry capabilities emitted by this producing build.
pub const CURRENT_UPDATER_CONTRACT: &[u8] = include_bytes!("updater.json");
/// Preparation takes an installation path and workspace path, with no inherited authority.
pub const PREPARATION_PROTOCOL: u32 = 1;
/// Public process-entry spelling retained for installed application-role compatibility.
pub const PREPARATION_ARGUMENT: &str = "--internal-update-preparation";

/// Bounded capabilities from verified payload bytes; decoding grants no execution authority.
#[derive(Debug)]
pub struct UpdaterContract {
    preparation_protocols: Vec<u32>,
    installation_schemas: Vec<u32>,
    probe_protocols: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparationWire {
    schema_version: u32,
    preparation_protocols: Vec<u32>,
    installation_schemas: Vec<u32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeWire {
    schema_version: u32,
    preparation_protocols: Vec<u32>,
    installation_schemas: Vec<u32>,
    probe_protocols: Vec<u32>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Wire {
    Preparation(PreparationWire),
    Probe(ProbeWire),
}
impl UpdaterContract {
    /// Decode explicit version sets without treating an executable name as compatibility.
    /// # Errors
    /// Rejects oversized, unknown, missing, zero, duplicated or unordered declarations.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_CONTRACT_BYTES {
            return Err(Rejection::Format);
        }
        let value = match serde_json::from_slice(bytes).map_err(|_| Rejection::Format)? {
            Wire::Preparation(value) if value.schema_version == 1 => Self {
                preparation_protocols: value.preparation_protocols,
                installation_schemas: value.installation_schemas,
                probe_protocols: Vec::new(),
            },
            Wire::Probe(value)
                if value.schema_version == 2 && valid_versions(&value.probe_protocols) =>
            {
                Self {
                    preparation_protocols: value.preparation_protocols,
                    installation_schemas: value.installation_schemas,
                    probe_protocols: value.probe_protocols,
                }
            }
            _ => return Err(Rejection::Format),
        };
        if !valid_versions(&value.preparation_protocols)
            || !valid_versions(&value.installation_schemas)
        {
            return Err(Rejection::Format);
        }
        Ok(value)
    }

    /// Admit the caller's actual entry and installation schema before creating a helper.
    /// # Errors
    /// Returns `InstallationSchema` or `Unsupported` without falling back to another executable.
    pub fn require_preparation(
        &self,
        protocol: u32,
        installation_schema: u32,
    ) -> Result<(), Rejection> {
        if !self.installation_schemas.contains(&installation_schema) {
            return Err(Rejection::InstallationSchema);
        }
        if !self.preparation_protocols.contains(&protocol) {
            return Err(Rejection::Unsupported);
        }
        Ok(())
    }

    /// Require an explicitly advertised probe before admitting a native engine trial.
    /// Version-one declarations retain preparation support and never gain probe capability.
    /// # Errors
    /// Rejects unsupported probe versions or installation schemas without guessing from filenames.
    pub fn require_probe(&self, protocol: u32, installation_schema: u32) -> Result<(), Rejection> {
        if !self.installation_schemas.contains(&installation_schema) {
            return Err(Rejection::InstallationSchema);
        }
        if !self.probe_protocols.contains(&protocol) {
            return Err(Rejection::Unsupported);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
