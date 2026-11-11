// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Declare the updater's entry and installation reader independently of desktop entry.
use super::{MAX_CONTRACT_BYTES, Rejection, valid_versions};
use serde::Deserialize;
use std::collections::BTreeMap;
pub mod activation;
pub mod probe;

/// Authenticated declaration accompanying the independently executable updater.
pub const UPDATER_CONTRACT_FILE: &str = "sugarsubstitute-updater.json";
/// Exact entry capabilities emitted by this producing build.
pub const CURRENT_UPDATER_CONTRACT: &[u8] = include_bytes!("updater.json");
/// Preparation takes an installation path and workspace path, with no inherited authority.
pub const PREPARATION_PROTOCOL: u32 = 1;
/// Public process-entry spelling retained for installed application-role compatibility.
pub const PREPARATION_ARGUMENT: &str = "--internal-update-preparation";
/// Startup activation offers verified launch custody to its separate native creator.
pub const ACTIVATION_PROTOCOL: u32 = 2;
/// Follow with an installation path and encoded `activation::ActivationRequest`.
pub const ACTIVATION_ARGUMENT: &str = "--internal-update-activation-v2";

/// Bounded capabilities from verified payload bytes; decoding grants no execution authority.
#[derive(Debug)]
pub struct UpdaterContract {
    preparation_protocols: Vec<u32>,
    installation_schemas: Vec<u32>,
    probe_protocols: Vec<u32>,
    activation_protocols: Vec<u32>,
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
#[serde(deny_unknown_fields)]
struct EntriesWire {
    schema_version: u32,
    installation_schemas: Vec<u32>,
    #[serde(deserialize_with = "unique_entries")]
    entry_protocols: BTreeMap<String, Vec<u32>>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Wire {
    Preparation(PreparationWire),
    Probe(ProbeWire),
    Entries(EntriesWire),
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
                activation_protocols: Vec::new(),
            },
            Wire::Probe(value)
                if value.schema_version == 2 && valid_versions(&value.probe_protocols) =>
            {
                Self {
                    preparation_protocols: value.preparation_protocols,
                    installation_schemas: value.installation_schemas,
                    probe_protocols: value.probe_protocols,
                    activation_protocols: Vec::new(),
                }
            }
            Wire::Entries(mut value)
                if value.schema_version == 3 && valid_entries(&value.entry_protocols) =>
            {
                Self {
                    preparation_protocols: value
                        .entry_protocols
                        .remove("prepare")
                        .ok_or(Rejection::Format)?,
                    installation_schemas: value.installation_schemas,
                    probe_protocols: value.entry_protocols.remove("probe").unwrap_or_default(),
                    activation_protocols: value
                        .entry_protocols
                        .remove("activate")
                        .unwrap_or_default(),
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

    /// Require explicit startup activation support. Historical preparation/probe declarations
    /// never acquire this capability. Unknown entry names do not grant executable commands.
    /// # Errors
    /// Rejects unsupported activation versions and installation schemas.
    pub fn require_activation(
        &self,
        protocol: u32,
        installation_schema: u32,
    ) -> Result<(), Rejection> {
        if !self.installation_schemas.contains(&installation_schema) {
            return Err(Rejection::InstallationSchema);
        }
        if !self.activation_protocols.contains(&protocol) {
            return Err(Rejection::Unsupported);
        }
        Ok(())
    }
}

/// Future entries remain readable by existing engines without becoming executable authority.
fn valid_entries(entries: &BTreeMap<String, Vec<u32>>) -> bool {
    !entries.is_empty()
        && entries.len() <= 16
        && entries.iter().all(|(name, versions)| {
            !name.is_empty()
                && name.len() <= 32
                && name.as_bytes()[0].is_ascii_lowercase()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && valid_versions(versions)
        })
}

/// Serde's default map decoder replaces duplicate keys; capability declarations reject them.
fn unique_entries<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<BTreeMap<String, Vec<u32>>, D::Error> {
    struct Entries;
    impl<'de> serde::de::Visitor<'de> for Entries {
        type Value = BTreeMap<String, Vec<u32>>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded map of unique entry names")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut entries = BTreeMap::new();
            while let Some((name, versions)) = map.next_entry::<String, Vec<u32>>()? {
                if entries.len() == 16 || entries.insert(name, versions).is_some() {
                    return Err(serde::de::Error::custom("Duplicate or excessive updater entries"));
                }
            }
            Ok(entries)
        }
    }
    decoder.deserialize_map(Entries)
}

#[cfg(test)]
mod tests;
