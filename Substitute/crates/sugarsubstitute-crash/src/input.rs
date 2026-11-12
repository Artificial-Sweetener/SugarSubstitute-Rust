// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound trusted host-composition inputs before task admission or native allocation.
use crate::CrashError;
use std::{
    fmt,
    path::{Component, Path, PathBuf},
};
#[cfg(windows)]
use std::{os::windows::ffi::OsStrExt, path::Prefix};
use sugarsubstitute_contracts::crash::CaptureOperation;

/// Actual original two-MiB capture owners plus the separately issued trusted bundle.
#[cfg(test)]
pub(crate) const BASE_STORAGE_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const STORAGE_BYTES: usize = 4 * 1024 * 1024;
#[cfg(windows)]
pub(crate) const SOURCE_STORAGE_BYTES: usize = 16 * 1024;
#[cfg(windows)]
pub(crate) const NATIVE_STORAGE_BYTES: usize = 966_656;
pub(crate) const MAX_UNITS: usize = 32766;
pub(crate) const MAX_ANCESTORS: usize = 128;
/// Retain the protected root and an inert current-process installed-release request.
/// Only genuine Core admission and the authenticated selector grant native effects.
/// Caller paths, digests and build labels cannot select executable capture files.
pub struct CaptureStartInput {
    pub(crate) capture_root: PathBuf,
    #[cfg(windows)]
    pub(crate) recovery:
        Option<sugarsubstitute_execution::recovery_supervisor::AuthenticatedRecoveryOrigin>,
    pub(crate) selection: Option<sugarsubstitute_supervision::CaptureSelectionRequest>,
}
impl CaptureStartInput {
    /// Retain the original root and inert request without allocating a trust projection.
    /// # Errors
    /// Rejects non-local/relative, parent, NUL, oversized paths/capacities.
    pub fn new(
        capture_root: PathBuf,
        selection: sugarsubstitute_supervision::CaptureSelectionRequest,
    ) -> Result<Self, CrashError> {
        if capture_root.capacity() > MAX_UNITS * 3 {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        validate_path(&capture_root)?;
        Ok(Self {
            capture_root,
            #[cfg(windows)]
            recovery: None,
            selection: Some(selection),
        })
    }
    /// Retain only the authenticated Root recovery source for this fresh process.
    /// Root's protected path is copied under the existing native-input reservation;
    /// registration independently verifies its actual pinned directory identity.
    /// # Errors
    /// Rejects a stopped genuine source or an invalid bounded local Root path.
    #[cfg(windows)]
    pub fn recovery(
        source: sugarsubstitute_execution::recovery_supervisor::AuthenticatedRecoveryOrigin,
        selection: sugarsubstitute_supervision::CaptureSelectionRequest,
    ) -> Result<Self, CrashError> {
        use std::os::windows::ffi::OsStringExt;
        source.check().map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let units: Vec<u16> = source.capture_root_units().collect();
        let path = PathBuf::from(std::ffi::OsString::from_wide(&units));
        let mut result = Self::new(path, selection)?;
        result.recovery = Some(source);
        Ok(result)
    }
    /// Reserve the original fixed claims and one actual two-MiB selected bundle claim.
    /// The original claims remain16384 source,966656 native/input,65536 callback,
    /// and1048576 protected root. Core alone issues the additional2097152 bundle.
    /// Native code/mappings and upstream runtime allocation are separate physical storage.
    pub const fn required_reservation_bytes(&self) -> usize {
        STORAGE_BYTES
    }
    /// Borrow the actual host-owned protected root; lexical shape grants no file trust.
    pub fn capture_root_path(&self) -> &Path {
        &self.capture_root
    }
}
impl fmt::Debug for CaptureStartInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureStartInput").finish_non_exhaustive()
    }
}
pub(crate) fn validate_path(path: &Path) -> Result<(), CrashError> {
    if !path.is_absolute()
        || path.as_os_str().len() > MAX_UNITS * 3
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path.components().count() > MAX_ANCESTORS
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    #[cfg(windows)]
    if !matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        || path.as_os_str().encode_wide().take(MAX_UNITS + 1).count() > MAX_UNITS
    {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    Ok(())
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
