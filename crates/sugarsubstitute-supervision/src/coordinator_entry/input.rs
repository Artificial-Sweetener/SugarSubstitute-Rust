// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded inert entry paths and the exact invoking launch snapshot.
use super::types::EntryInputError;
use std::path::PathBuf;
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration, coordinator_entry::EntryLaunchRequest,
};
use sugarsubstitute_execution::EntryRequestInput;

/// Own inert namespace-discovery paths and one original launch request.
/// Native discovery independently validates installation and running-image identity.
pub struct CoordinatorEntryInput {
    pub(super) installation: PathBuf,
    pub(super) executable: PathBuf,
    pub(super) request: Option<EntryRequestInput>,
}
impl CoordinatorEntryInput {
    /// Preserve original bounded paths and request without native effects.
    /// # Errors
    /// Rejects invalid absolute Windows paths or excessive retained path capacities.
    pub fn new(
        installation: PathBuf,
        executable: PathBuf,
        request: EntryLaunchRequest,
    ) -> Result<Self, EntryInputError> {
        Self::from_owned(installation, executable, request.into())
    }
    /// Retain complete original prepared data without separating its custody.
    /// # Errors
    /// Preserves existing absolute-path grammar and retained path-capacity limits.
    pub fn from_owned(
        installation: PathBuf,
        executable: PathBuf,
        request: EntryRequestInput,
    ) -> Result<Self, EntryInputError> {
        LaunchConfiguration::validate_path(&installation)
            .map_err(EntryInputError::Configuration)?;
        LaunchConfiguration::validate_path(&executable).map_err(EntryInputError::Configuration)?;
        if installation
            .capacity()
            .checked_add(executable.capacity())
            .is_none_or(|bytes| bytes > 256 * 1024)
        {
            return Err(EntryInputError::Capacity);
        }
        Ok(Self { installation, executable, request: Some(request) })
    }
}
