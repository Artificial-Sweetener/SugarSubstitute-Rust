// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advertise native application entry protocols inside the authenticated package inventory.
use serde::Deserialize;
pub mod installation;
mod launcher;
pub mod updater;
pub use launcher::{CURRENT_LAUNCHER_CONTRACT, LAUNCHER_CONTRACT_FILE, LauncherContract};

/// Filename beneath the authenticated application's payload directory.
pub const CONTRACT_FILE: &str = "sugarsubstitute-application.json";
/// Current inherited selection protocol implemented by the native entry boundary.
///
/// The transferred selection carries installation selection schema 2, whose generations name
/// their signed release. Protocols 1 (accepted) and 2 (trial) carried schema 1; a launcher and
/// application compare declared numbers before creation, so a skewed pair is refused instead
/// of exchanging records the receiver would misread.
pub const INSTALLED_LAUNCH_PROTOCOL: u32 = 3;
/// Versioned entry argument; the following value names the explicitly inherited selection handle.
pub const INSTALLED_LAUNCH_ARGUMENT: &str = "--installed-launch-v3";
/// Trial entry carries explicit run and snapshot handles instead of accepted-only selection.
/// Its snapshot holds a selection schema 2 record, like `INSTALLED_LAUNCH_PROTOCOL`.
pub const TRIAL_LAUNCH_PROTOCOL: u32 = 4;
/// Versioned trial entry argument followed by run and snapshot handle values, in that order.
pub const TRIAL_LAUNCH_ARGUMENT: &str = "--installed-launch-v4";
/// Exact contract distributed with applications built from this source.
pub const CURRENT_CONTRACT: &[u8] = include_bytes!("application_launch/current.json");
/// Bound before decoding untrusted application compatibility metadata.
pub const MAX_CONTRACT_BYTES: usize = 4096;

/// Bounded protocol declaration. Decoding does not authenticate the file or authorize execution.
#[derive(Debug)]
pub struct ApplicationContract {
    installed_launch_protocols: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContract {
    schema_version: u32,
    installed_launch_protocols: Vec<u32>,
}

/// An application cannot use this launcher's entry contract.
#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Malformed, oversized, ambiguous or unsupported contract format.
    Format,
    /// The package predates explicit launch compatibility metadata.
    Missing,
    /// A valid application requires another launch protocol.
    Unsupported,
    /// The launcher cannot read this installation selection schema.
    InstallationSchema,
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Native launch compatibility rejected: {self:?}")
    }
}
impl std::error::Error for Rejection {}

impl ApplicationContract {
    /// Decode explicit, ordered supported versions without guessing capabilities from a filename.
    /// # Errors
    /// Rejects unknown fields/schema, zero, duplicates, unordered versions and oversized input.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_CONTRACT_BYTES {
            return Err(Rejection::Format);
        }
        let value: WireContract = serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?;
        if value.schema_version != 1 || !valid_versions(&value.installed_launch_protocols) {
            return Err(Rejection::Format);
        }
        Ok(Self { installed_launch_protocols: value.installed_launch_protocols })
    }

    /// Require the actual protocol implemented by the caller; metadata cannot choose a command.
    /// # Errors
    /// Returns `Unsupported` when a valid declaration does not include this protocol.
    pub fn require_installed_launch(&self, protocol: u32) -> Result<(), Rejection> {
        if self.installed_launch_protocols.contains(&protocol) {
            Ok(())
        } else {
            Err(Rejection::Unsupported)
        }
    }

    /// Require an entry suitable for an already accepted installation.
    /// # Errors
    /// Rejects trial-only entry and protocols absent from the application's declaration.
    pub fn require_accepted_launch(&self, protocol: u32) -> Result<(), Rejection> {
        if protocol == TRIAL_LAUNCH_PROTOCOL {
            return Err(Rejection::Unsupported);
        }
        self.require_installed_launch(protocol)
    }
}

/// Canonical bounded version sets keep declarations unambiguous without version-range guesses.
fn valid_versions(versions: &[u32]) -> bool {
    !versions.is_empty()
        && versions.len() <= 16
        && versions[0] != 0
        && versions.windows(2).all(|pair| pair[0] < pair[1])
}

#[cfg(test)]
mod tests;
