// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse explicit inert installation inputs before execution or native effects.

use std::{error::Error, ffi::OsString, fmt, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::{ConfigurationError, LaunchConfiguration},
    coordinator_entry::MAX_ENTRY_REQUEST_ALLOCATION_BYTES,
};

/// Select explicit caller paths or the installed descriptor read after splash presentation.
pub(crate) enum ConfigurationSource {
    Explicit(ProductionEntryOptions),
    Installed(PathBuf),
}
impl From<ProductionEntryOptions> for ConfigurationSource {
    fn from(paths: ProductionEntryOptions) -> Self {
        Self::Explicit(paths)
    }
}

/// Keep the original six separately selected paths without discovering an install.
pub(crate) struct ProductionEntryOptions {
    pub(crate) configuration: PathBuf,
    pub(crate) package: PathBuf,
    pub(crate) metadata: PathBuf,
    pub(crate) cubes: PathBuf,
    pub(crate) script: PathBuf,
    pub(crate) profile: PathBuf,
}

/// Reject command shape without formatting supplied paths or argument values.
#[derive(Debug)]
pub(crate) enum ProductionOptionsError {
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
impl fmt::Display for ProductionOptionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Production launch options are invalid")
    }
}
impl Error for ProductionOptionsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Path(cause) => Some(cause),
            _ => None,
        }
    }
}
impl ProductionEntryOptions {
    /// Consume exactly six required path options, preserving original Windows units.
    ///
    /// This performs lexical validation only. Native filesystem, package and
    /// process authority belong to their later retained adapters.
    /// # Errors
    /// Rejects missing, duplicate, unknown or valueless options, invalid paths and
    /// aggregate actual path capacity above the existing100KiB request bound.
    pub(crate) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, ProductionOptionsError> {
        let mut arguments = arguments.into_iter();
        let mut paths: [Option<PathBuf>; 6] = std::array::from_fn(|_| None);
        let mut retained = 0_u64;
        while let Some(option) = arguments.next() {
            let index = match option.to_str() {
                Some("--startup-config") => 0,
                Some("--frontend-package") => 1,
                Some("--frontend-metadata") => 2,
                Some("--sugarcubes-root") => 3,
                Some("--backend-script") => 4,
                Some("--profile-directory") => 5,
                _ => return Err(ProductionOptionsError::Unknown),
            };
            if paths[index].is_some() {
                return Err(ProductionOptionsError::Duplicate);
            }
            let path = PathBuf::from(arguments.next().ok_or(ProductionOptionsError::MissingValue)?);
            LaunchConfiguration::validate_path(&path).map_err(ProductionOptionsError::Path)?;
            retained = retained
                .checked_add(
                    u64::try_from(path.capacity()).map_err(|_| ProductionOptionsError::Capacity)?,
                )
                .filter(|total| *total <= MAX_ENTRY_REQUEST_ALLOCATION_BYTES)
                .ok_or(ProductionOptionsError::Capacity)?;
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
            return Err(ProductionOptionsError::Missing);
        };
        Ok(Self { configuration, package, metadata, cubes, script, profile })
    }
}

#[cfg(test)]
#[path = "options/tests.rs"]
mod tests;
