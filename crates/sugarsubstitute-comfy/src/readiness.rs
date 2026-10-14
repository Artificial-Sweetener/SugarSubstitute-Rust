// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode observed ComfyUI HTTP layouts without granting listener or process trust.
//! `server.py` serves `/system_stats` as a system/devices object and `/extensions`
//! as an array of extension URLs. Neither endpoint declares a wire schema version.
//! These checks establish response shape and matching expected metadata only;
//! managed readiness additionally requires retained root and actual listener proof.
use serde::Deserialize;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::configuration::LaunchConfiguration;
mod extensions;
mod identity;
mod json;
mod schema;
pub use identity::InstalledFrontendIdentity;
pub use schema::{PackageVersion, SystemStats};

/// Maximum complete `/system_stats` body before any JSON allocation.
pub const MAX_SYSTEM_STATS_BYTES: usize = 64 * 1024;
/// Maximum complete `/extensions` body, independently of control-message limits.
pub const MAX_EXTENSIONS_BYTES: usize = 1024 * 1024;
/// Maximum extension entries, including extensions outside SugarCubes.
pub const MAX_EXTENSION_PATHS: usize = 4096;
/// Maximum UTF-8 bytes in one served extension path.
pub const MAX_EXTENSION_PATH_BYTES: usize = 1024;

/// A bounded response or expected identity failed structural or comparison checks.
pub enum ReadinessError {
    /// Reject the entire body before parsing.
    TooLarge,
    /// Retain the original JSON cause while redacting supplied response text.
    Json(serde_json::Error),
    /// A field, count, path, version declaration or duplicate entry is invalid.
    Shape,
    /// The served frontend installed version differs from the expected installation.
    FrontendMismatch,
    /// At least one required installed SugarCubes URL is absent.
    MissingExtension,
    /// The observed server argv differs from the complete managed script argv.
    ManagedArgumentsMismatch,
}
impl fmt::Display for ReadinessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "ComfyUI response exceeds its endpoint byte bound",
            Self::Json(_) => "ComfyUI response contains invalid or duplicate JSON data",
            Self::Shape => "ComfyUI readiness metadata has an unsupported shape",
            Self::FrontendMismatch => "Served frontend differs from the expected installation",
            Self::MissingExtension => "Served extensions omit required SugarCubes paths",
            Self::ManagedArgumentsMismatch => "Served argv differs from the managed command",
        })
    }
}
impl fmt::Debug for ReadinessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl Error for ReadinessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Retain admitted extension URLs; this is observed data, never installed-file proof.
#[derive(Clone, Eq, PartialEq)]
pub struct ExtensionInventory {
    paths: Vec<String>,
}
impl ExtensionInventory {
    /// Borrow exact served URL spellings, without filesystem or fetch authority.
    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}
impl fmt::Debug for ExtensionInventory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionInventory").field("count", &self.paths.len()).finish()
    }
}

/// Decode the observed system layout with duplicate-field and complete-body validation.
/// Unknown supplemental fields are allowed inside the bounded JSON envelope;
/// a declared `schema_version` is rejected because this endpoint defines none.
pub fn decode_system_stats(bytes: &[u8]) -> Result<SystemStats, ReadinessError> {
    json::decode(bytes, MAX_SYSTEM_STATS_BYTES)?;
    let wire = schema::Response::deserialize(&mut serde_json::Deserializer::from_slice(bytes))
        .map_err(ReadinessError::Json)?;
    if wire.devices.len() > 64 {
        return Err(ReadinessError::Shape);
    }
    wire.system.validate()
}

/// Decode complete served extension URLs, rejecting duplicates and ambiguous paths.
/// Values must be absolute `/extensions/.../*.js` paths, with no traversal,
/// backslashes, query, fragment or encoded path separators/dot segments.
pub fn decode_extensions(bytes: &[u8]) -> Result<ExtensionInventory, ReadinessError> {
    json::decode(bytes, MAX_EXTENSIONS_BYTES)?;
    let paths = extensions::decode(bytes)?;
    identity::validate_paths(&paths, false)?;
    Ok(ExtensionInventory { paths })
}

/// Compare only server argv against an already effective managed command.
/// ComfyUI's `sys.argv` excludes the Python `-u` switch and includes `main.py`.
/// Call this separately from installed identity checks for managed connections;
/// attached connections do not require equality to a command owned by this host.
/// Even exact equality does not authenticate the peer or grant process authority.
pub fn compare_managed_arguments(
    stats: &SystemStats,
    effective: &LaunchConfiguration,
) -> Result<(), ReadinessError> {
    let Some((first, argv)) = effective.arguments().split_first() else {
        return Err(ReadinessError::ManagedArgumentsMismatch);
    };
    if first != "-u" || stats.argv() != argv {
        return Err(ReadinessError::ManagedArgumentsMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
