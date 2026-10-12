// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own exact invocation inputs and measure retained capacity before adapter admission.

use super::{EntryShapeError, validate_environment, validate_path_units};
use crate::configuration::{ConnectionConfiguration, LaunchConfiguration, StartupConfiguration};
use std::{
    fmt,
    path::{Path, PathBuf},
};

/// Maximum dynamic capacity retained by one inert owned invocation.
pub const MAX_ENTRY_REQUEST_ALLOCATION_BYTES: u64 = 100 * 1024;

/// Explicit startup intent retaining exact user paths, arguments and environment order.
/// Construction performs no native inspection or authority admission.
pub struct EntryLaunchRequest {
    parts: EntryLaunchParts,
    retained: u64,
}
/// Transfer original owned inputs after application capacity admission, without copying.
pub struct EntryLaunchParts {
    /// Already validated startup configuration.
    pub configuration: StartupConfiguration,
    /// Requested installed frontend package directory.
    pub package: PathBuf,
    /// Requested frontend distribution METADATA file.
    pub metadata: PathBuf,
    /// Requested SugarCubes directory.
    pub cube: PathBuf,
    /// Requested ComfyUI backend script.
    pub script: PathBuf,
    /// Requested browser profile directory.
    pub profile_directory: PathBuf,
    /// Exact original Windows environment snapshot including double NUL.
    pub environment: Vec<u16>,
}
impl EntryLaunchRequest {
    /// Retain explicit inputs and bound every actual dynamic allocation capacity.
    /// Paths remain lexical until native adapters independently pin their identity.
    /// # Errors
    /// Rejects path/environment shape or retained capacity above 100 KiB.
    #[allow(
        clippy::too_many_arguments,
        reason = "Four independently selected frontend paths remain explicit at the inert contract boundary"
    )]
    pub fn new(
        configuration: StartupConfiguration,
        package: PathBuf,
        metadata: PathBuf,
        cube: PathBuf,
        script: PathBuf,
        profile_directory: PathBuf,
        environment: Vec<u16>,
    ) -> Result<Self, EntryShapeError> {
        let parts = EntryLaunchParts {
            configuration,
            package,
            metadata,
            cube,
            script,
            profile_directory,
            environment,
        };
        let mut retained = 0_u64;
        for path in
            [&parts.package, &parts.metadata, &parts.cube, &parts.script, &parts.profile_directory]
        {
            validate_owned_path(path)?;
            add_capacity(&mut retained, path.capacity())?;
        }
        if let ConnectionConfiguration::Launch(launch) = parts.configuration.connection() {
            for path in [launch.root(), launch.interpreter()] {
                LaunchConfiguration::validate_path(path).map_err(EntryShapeError::Configuration)?;
            }
            // Path getters cannot expose spare allocation; actual capacities are owned metadata.
            add_capacity(
                &mut retained,
                launch.allocated_bytes().ok_or(EntryShapeError::Capacity)?,
            )?;
        }
        add_capacity(&mut retained, parts.configuration.profile().allocated_bytes())?;
        add_capacity(
            &mut retained,
            parts
                .environment
                .capacity()
                .checked_mul(size_of::<u16>())
                .ok_or(EntryShapeError::Capacity)?,
        )?;
        validate_environment(parts.environment.iter().copied())?;
        if retained > MAX_ENTRY_REQUEST_ALLOCATION_BYTES {
            return Err(EntryShapeError::Capacity);
        }
        Ok(Self { parts, retained })
    }
    /// Borrow inert configuration, independently of native authority.
    pub fn configuration(&self) -> &StartupConfiguration {
        &self.parts.configuration
    }
    /// Borrow the original requested frontend package directory.
    pub fn package_directory(&self) -> &Path {
        &self.parts.package
    }
    /// Borrow the original requested frontend METADATA file.
    pub fn metadata_file(&self) -> &Path {
        &self.parts.metadata
    }
    /// Borrow the original requested SugarCubes directory.
    pub fn cube_directory(&self) -> &Path {
        &self.parts.cube
    }
    /// Borrow the original requested ComfyUI script.
    pub fn backend_script(&self) -> &Path {
        &self.parts.script
    }
    /// Borrow the original requested browser profile directory.
    pub fn profile_directory(&self) -> &Path {
        &self.parts.profile_directory
    }
    /// Borrow exact snapshot units including the original double terminator.
    pub fn environment(&self) -> &[u16] {
        &self.parts.environment
    }
    /// Return checked retained dynamic capacity in bytes, excluding fixed Self storage.
    pub const fn retained_bytes(&self) -> u64 {
        self.retained
    }
    /// Move original inputs without reallocating or normalizing user content.
    pub fn into_parts(self) -> EntryLaunchParts {
        self.parts
    }
}
impl fmt::Debug for EntryLaunchRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryLaunchRequest")
            .field("retained_bytes", &self.retained)
            .finish_non_exhaustive()
    }
}
fn add_capacity(total: &mut u64, bytes: usize) -> Result<(), EntryShapeError> {
    *total = total
        .checked_add(u64::try_from(bytes).map_err(|_| EntryShapeError::Capacity)?)
        .ok_or(EntryShapeError::Capacity)?;
    Ok(())
}
fn validate_owned_path(path: &Path) -> Result<(), EntryShapeError> {
    if !path.is_absolute() {
        return Err(EntryShapeError::Path);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        validate_path_units(path.as_os_str().encode_wide())
    }
    #[cfg(not(target_os = "windows"))]
    {
        validate_path_units(path.to_str().ok_or(EntryShapeError::Path)?.encode_utf16())
    }
}
