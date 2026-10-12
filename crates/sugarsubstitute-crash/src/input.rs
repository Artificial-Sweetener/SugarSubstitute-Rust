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

pub(crate) const STORAGE_BYTES: usize = 2 * 1024 * 1024;
#[cfg(windows)]
pub(crate) const SOURCE_STORAGE_BYTES: usize = 16 * 1024;
#[cfg(windows)]
pub(crate) const NATIVE_STORAGE_BYTES: usize =
    STORAGE_BYTES - SOURCE_STORAGE_BYTES - 64 * 1024 - 1024 * 1024;
pub(crate) const MAX_UNITS: usize = 32766;
pub(crate) const MAX_ANCESTORS: usize = 128;

/// Exact packaged capture files and an existing protected host-owned capture root.
///
/// This is trusted local composition, not an IPC authority grant. The native
/// adapter pins non-reparse ancestors/targets and verifies exact file hashes.
/// No argument, URL, environment, upload or cleanup-path override is accepted.
/// Paths contain at most 32766 UTF-16 units/128 components and bounded capacities.
pub struct CaptureStartInput {
    pub(crate) client: PathBuf,
    pub(crate) handler: PathBuf,
    pub(crate) capture_root: PathBuf,
    pub(crate) client_hash: [u8; 32],
    pub(crate) handler_hash: [u8; 32],
    pub(crate) build: String,
}
impl CaptureStartInput {
    /// Validate inputs without filesystem effects or cloning their owned paths.
    ///
    /// # Errors
    /// Rejects non-local/relative, parent, NUL, oversized paths/capacities and
    /// build identities outside 1..=64 ASCII alphanumeric/underscore/hyphen bytes.
    pub fn new(
        client: PathBuf,
        handler: PathBuf,
        capture_root: PathBuf,
        client_hash: [u8; 32],
        handler_hash: [u8; 32],
        build: &str,
    ) -> Result<Self, CrashError> {
        for path in [&client, &handler, &capture_root] {
            if path.capacity() > MAX_UNITS * 3 {
                return Err(CrashError::invalid(CaptureOperation::Runtime));
            }
            validate_path(path)?;
        }
        if build.is_empty()
            || build.len() > 64
            || !build
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        Ok(Self {
            client,
            handler,
            capture_root,
            client_hash,
            handler_hash,
            build: build.to_owned(),
        })
    }
    /// Reserve first-party input/scratch/retained native storage before effects.
    ///
    /// The fixed 2MiB assigns 16KiB to registration-lifetime replay/source storage,
    /// 966656 bytes to bounded input/native scratch, 64KiB to process-pinned
    /// callbacks and 1MiB to protected generation storage. Native storage covers
    /// three input path capacities, bounded UTF16/canonical conversion, 1028
    /// retained ancestor/target/partial handles and hashing scratch.
    /// Crashpad's vendored runtime allocations/mapped code are separate native
    /// allocations; this value does not claim an OS working-set limit.
    pub const fn required_reservation_bytes(&self) -> usize {
        STORAGE_BYTES
    }
    /// Borrow the exact trusted bridge path; this data alone grants no filesystem trust.
    pub fn client_path(&self) -> &Path {
        &self.client
    }
    /// Borrow the exact packaged handler path.
    pub fn handler_path(&self) -> &Path {
        &self.handler
    }
    /// Borrow the existing protected host root; native resolution grants trust.
    /// The owner exclusively creates this coordinator's database/metrics generation.
    pub fn capture_root_path(&self) -> &Path {
        &self.capture_root
    }
    /// Borrow the bounded ASCII build annotation.
    pub fn build_identity(&self) -> &str {
        &self.build
    }
    /// Return the trusted expected bridge content digest; equality is checked natively.
    pub const fn client_sha256(&self) -> &[u8; 32] {
        &self.client_hash
    }
    /// Return the trusted expected handler content digest; equality is checked natively.
    pub const fn handler_sha256(&self) -> &[u8; 32] {
        &self.handler_hash
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
