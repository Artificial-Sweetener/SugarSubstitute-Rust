// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Versioned installed launch inputs; generation binding remains with native storage.
use crate::{
    paths::{INPUT_OPTIONS, StartupPaths},
    startup::MAX_STARTUP_CONFIGURATION_BYTES,
};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, io, path::Path};

/// Exact installed argument spelling, decoded without obtaining filesystem authority.
pub struct LaunchDescriptor {
    schema_version: u32,
    arguments: Vec<String>,
}

/// Decode wire fields privately so deserialization cannot bypass version checks.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    arguments: Vec<String>,
}

/// Only this bounded encoder can serialize a descriptor; callers receive typed intent.
#[derive(Serialize)]
struct Encoded<'a> {
    schema_version: u32,
    arguments: &'a [String],
}

impl LaunchDescriptor {
    /// Decode bounded JSON with explicit version and fields. Runtime and maintenance
    /// validate their supported input contracts separately before using the descriptor.
    /// # Errors
    /// Rejects size, malformed JSON, unknown fields and unsupported versions.
    pub fn parse(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() > MAX_STARTUP_CONFIGURATION_BYTES {
            return Err(invalid("Installed launch descriptor exceeds its byte bound"));
        }
        let descriptor: Wire = serde_json::from_slice(bytes).map_err(io::Error::other)?;
        if descriptor.schema_version != 1 {
            return Err(invalid("Unsupported installed launch descriptor"));
        }
        Ok(Self { schema_version: descriptor.schema_version, arguments: descriptor.arguments })
    }

    /// Validate the same six inputs used by explicit production invocation, moving
    /// each original string into its path without discovering or trusting an installation.
    /// # Errors
    /// Rejects missing, duplicate or unknown options and invalid/budget-exceeding paths.
    pub fn into_startup_paths(self) -> Result<StartupPaths, crate::paths::StartupPathsError> {
        StartupPaths::parse(self.arguments.into_iter().map(OsString::from))
    }

    /// Encode validated initial inputs in canonical option order.
    /// # Errors
    /// Rejects non-Unicode installed paths and oversized serialized documents.
    pub fn encode(paths: StartupPaths) -> io::Result<Vec<u8>> {
        let mut arguments = Vec::new();
        for (flag, path) in INPUT_OPTIONS.into_iter().zip(paths.into_paths()) {
            arguments.push(flag.to_owned());
            arguments.push(
                path.into_os_string()
                    .into_string()
                    .map_err(|_| invalid("Installed configuration path is not Unicode"))?,
            );
        }
        Self { schema_version: 1, arguments }.bytes()
    }

    /// Validate the persisted maintenance contract and return its configuration slot.
    /// This preserves the historical 4096 UTF-8 byte limit per saved path. The
    /// caller must bind that slot to the retained native generation before relocation.
    fn maintenance_configuration_index(&self) -> io::Result<usize> {
        if self.arguments.len() != INPUT_OPTIONS.len() * 2 {
            return Err(invalid("Saved launch descriptor requires a compatible installer"));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut configuration = None;
        for (index, pair) in self.arguments.chunks_exact(2).enumerate() {
            if !INPUT_OPTIONS.contains(&pair[0].as_str())
                || !seen.insert(&pair[0])
                || pair[1].len() > 4096
            {
                return Err(invalid("Saved launch descriptor contains unsupported options"));
            }
            sugarsubstitute_contracts::configuration::LaunchConfiguration::validate_path(
                Path::new(&pair[1]),
            )
            .map_err(io::Error::other)?;
            if pair[0] == INPUT_OPTIONS[0] {
                configuration = Some(index * 2 + 1);
            }
        }
        configuration.ok_or_else(|| invalid("Saved launch configuration is absent"))
    }

    /// Apply the storage owner's native path policy to every maintenance input.
    /// Configuration validates schema and option shape; this callback preserves
    /// platform-specific local-folder, component and namespace restrictions.
    /// # Errors
    /// Returns malformed descriptor errors or the original native validation cause.
    pub fn validate_maintenance_paths(
        &self,
        mut validate: impl FnMut(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        self.maintenance_configuration_index()?;
        for pair in self.arguments.chunks_exact(2) {
            validate(Path::new(&pair[1]))?;
        }
        Ok(())
    }

    /// Borrow a validated maintenance descriptor's configuration selection for native binding.
    /// # Errors
    /// Rejects descriptors outside the maintenance contract.
    pub fn maintenance_configuration(&self) -> io::Result<&Path> {
        Ok(Path::new(&self.arguments[self.maintenance_configuration_index()?]))
    }

    /// Preserve exact argument order and text while changing only a generation-local input.
    /// # Errors
    /// Rejects unsupported maintenance inputs or an invalid/non-Unicode replacement path.
    /// The caller is responsible for native generation binding and retained file custody.
    pub fn relocated(&self, configuration: &Path) -> io::Result<Self> {
        let index = self.maintenance_configuration_index()?;
        sugarsubstitute_contracts::configuration::LaunchConfiguration::validate_path(configuration)
            .map_err(io::Error::other)?;
        let mut arguments = self.arguments.clone();
        arguments[index] = configuration
            .to_str()
            .ok_or_else(|| invalid("Installed configuration path is not Unicode"))?
            .to_owned();
        Ok(Self { schema_version: 1, arguments })
    }

    /// Compare exact persisted arguments, retaining order and spelling.
    pub fn same_arguments(&self, other: &Self) -> bool {
        self.arguments == other.arguments
    }

    /// Compare validated maintenance inputs independently of option order and the
    /// generation-local startup path. No other spelling is normalized.
    pub fn same_external_inputs(&self, other: &Self) -> bool {
        fn external(descriptor: &LaunchDescriptor) -> std::collections::BTreeMap<&str, &str> {
            descriptor
                .arguments
                .chunks_exact(2)
                .filter(|pair| pair[0] != INPUT_OPTIONS[0])
                .map(|pair| (pair[0].as_str(), pair[1].as_str()))
                .collect()
        }
        self.maintenance_configuration_index().is_ok()
            && other.maintenance_configuration_index().is_ok()
            && external(self) == external(other)
    }

    /// Serialize the versioned descriptor without rewriting selected paths.
    /// # Errors
    /// Rejects serialization failure or a document above the supported byte limit.
    pub fn bytes(&self) -> io::Result<Vec<u8>> {
        StartupPaths::parse(self.arguments.iter().map(OsString::from)).map_err(io::Error::other)?;
        let bytes = serde_json::to_vec(&Encoded {
            schema_version: self.schema_version,
            arguments: &self.arguments,
        })
        .map_err(io::Error::other)?;
        if bytes.len() > MAX_STARTUP_CONFIGURATION_BYTES {
            return Err(invalid("Installed launch descriptor exceeds its byte bound"));
        }
        Ok(bytes)
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
