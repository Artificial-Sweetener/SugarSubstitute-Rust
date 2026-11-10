// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse explicit inert installation inputs before execution or native effects.

use std::{error::Error, ffi::OsString, fmt, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::{ConfigurationError, LaunchConfiguration},
    coordinator_entry::MAX_ENTRY_REQUEST_ALLOCATION_BYTES,
};

/// Canonical option order shared by explicit arguments and installed descriptors.
pub const INPUT_OPTIONS: [&str; 6] = [
    "--startup-config",
    "--frontend-package",
    "--frontend-metadata",
    "--sugarcubes-root",
    "--backend-script",
    "--profile-directory",
];

/// Inert startup selection retained until an admitted adapter reads installed launch documents.
/// This grants no installation, filesystem, backend or mutable-data ownership.
pub enum ConfigurationSource {
    /// Six explicitly selected, lexically validated paths.
    Explicit(StartupPaths),
    /// A descriptor whose bounded contents the startup storage owner must resolve and validate.
    Installed(PathBuf),
}
impl From<StartupPaths> for ConfigurationSource {
    fn from(paths: StartupPaths) -> Self {
        Self::Explicit(paths)
    }
}

/// Keep the original six separately selected paths without discovering an install.
pub struct StartupPaths {
    configuration: PathBuf,
    package: PathBuf,
    metadata: PathBuf,
    cubes: PathBuf,
    script: PathBuf,
    profile: PathBuf,
}

/// Reject command shape without formatting supplied paths or argument values.
#[derive(Debug)]
pub enum StartupPathsError {
    /// A required path option was absent.
    Missing,
    /// An option occurred more than once.
    Duplicate,
    /// An unrecognized option or positional argument was supplied.
    Unknown,
    /// A recognized option had no following value.
    MissingValue,
    /// The authoritative inert path validator rejected its original spelling.
    Path(ConfigurationError),
    /// Original retained input capacities exceed the existing invocation bound.
    Capacity,
}
impl fmt::Display for StartupPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Production launch options are invalid")
    }
}
impl Error for StartupPathsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Path(cause) => Some(cause),
            _ => None,
        }
    }
}
impl StartupPaths {
    /// Validate six separately selected inputs without resolving or opening them.
    /// # Errors
    /// Rejects ambiguous paths and actual retained capacity above the entry request budget.
    pub fn new(paths: [PathBuf; 6]) -> Result<Self, StartupPathsError> {
        Self::parse(
            INPUT_OPTIONS
                .into_iter()
                .zip(paths)
                .flat_map(|(flag, path)| [OsString::from(flag), path.into_os_string()]),
        )
    }
    /// Borrow the selected input document without granting file authority.
    pub fn configuration(&self) -> &std::path::Path {
        &self.configuration
    }
    /// Borrow the six exact inert selections in option order for bounded passive capture.
    /// This performs no normalization, native inspection or authority admission.
    pub fn as_paths(&self) -> [&std::path::Path; 6] {
        [
            &self.configuration,
            &self.package,
            &self.metadata,
            &self.cubes,
            &self.script,
            &self.profile,
        ]
    }
    /// Move validated inputs in option order into the admitted native request constructor.
    /// These paths remain inert requests and require resource-owner trust checks.
    pub fn into_paths(self) -> [PathBuf; 6] {
        [self.configuration, self.package, self.metadata, self.cubes, self.script, self.profile]
    }
    /// Consume exactly six required path options, preserving original Windows units.
    ///
    /// This performs lexical validation only. Native filesystem, package and
    /// process authority belong to their later retained adapters.
    /// # Errors
    /// Rejects missing, duplicate, unknown or valueless options, invalid paths and
    /// aggregate actual path capacity above the existing 100 KiB request bound.
    pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, StartupPathsError> {
        let mut arguments = arguments.into_iter();
        let mut paths: [Option<PathBuf>; 6] = std::array::from_fn(|_| None);
        let mut retained = 0_u64;
        while let Some(option) = arguments.next() {
            let index = option
                .to_str()
                .and_then(|option| INPUT_OPTIONS.iter().position(|known| *known == option))
                .ok_or(StartupPathsError::Unknown)?;
            if paths[index].is_some() {
                return Err(StartupPathsError::Duplicate);
            }
            let path = PathBuf::from(arguments.next().ok_or(StartupPathsError::MissingValue)?);
            LaunchConfiguration::validate_path(&path).map_err(StartupPathsError::Path)?;
            retained = retained
                .checked_add(
                    u64::try_from(path.capacity()).map_err(|_| StartupPathsError::Capacity)?,
                )
                .filter(|total| *total <= MAX_ENTRY_REQUEST_ALLOCATION_BYTES)
                .ok_or(StartupPathsError::Capacity)?;
            paths[index] = Some(path);
        }
        let [
            Some(configuration),
            Some(package),
            Some(metadata),
            Some(cubes),
            Some(script),
            Some(profile),
        ] = paths
        else {
            return Err(StartupPathsError::Missing);
        };
        Ok(Self { configuration, package, metadata, cubes, script, profile })
    }
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
