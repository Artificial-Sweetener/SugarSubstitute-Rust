// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound claimed installation locations and immutable native inspection observations.
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// Explicit independently selected local sources; HTTP cannot manufacture this input.
/// Paths are lexical until the admitted native inspection resolves and pins them.
pub struct FrontendInstallationInput {
    pub(crate) package: PathBuf,
    pub(crate) metadata: PathBuf,
    pub(crate) cube: PathBuf,
    pub(crate) script: PathBuf,
}
impl FrontendInstallationInput {
    /// Select an installed frontend package, its METADATA file and SugarCubes root.
    /// Preserves exact spelling. Performs no filesystem discovery, import or effect.
    /// # Errors
    /// Rejects relative/NUL/oversized paths or excessive retained capacities.
    pub fn new(
        package: PathBuf,
        metadata: PathBuf,
        cube: PathBuf,
        script: PathBuf,
    ) -> Result<Self, FrontendSourceError> {
        for path in [&package, &metadata, &cube, &script] {
            if !path.is_absolute()
                || path.capacity() > 128 * 1024
                || path.as_os_str().as_encoded_bytes().contains(&0)
            {
                return Err(FrontendSourceError::invalid("frontend_location_shape"));
            }
        }
        Ok(Self { package, metadata, cube, script })
    }
    /// Borrow the selected package directory; this is not a canonical proof.
    pub fn package_directory(&self) -> &Path {
        &self.package
    }
}

/// One actual retained runtime file and its exact served URL/hash/length observation.
pub struct FrontendAsset {
    pub(crate) path: String,
    pub(crate) bytes: u64,
    pub(crate) hash: [u8; 32],
    pub(crate) identity: [u8; 24],
}
impl FrontendAsset {
    /// Borrow the fixed origin-relative URL; no arbitrary request authority is granted.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Return the complete expected file length in bytes.
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
    /// Return SHA-256 of the actual retained file bytes, not a RECORD/version claim.
    pub const fn sha256(&self) -> [u8; 32] {
        self.hash
    }
}
impl fmt::Debug for FrontendAsset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrontendAsset").field("bytes", &self.bytes).finish_non_exhaustive()
    }
}

/// Immutable actual-file observations held by an installed source lease.
pub struct FrontendSourceManifest {
    pub(crate) frontend: String,
    pub(crate) cube: String,
    pub(crate) assets: Vec<FrontendAsset>,
    pub(crate) cube_paths: Vec<String>,
    pub(crate) script: PathBuf,
    pub(crate) script_hash: [u8; 32],
}
impl FrontendSourceManifest {
    /// Borrow the actual canonical retained backend script path.
    pub fn backend_script(&self) -> &Path {
        &self.script
    }
    /// Return SHA-256 of the complete independently retained backend script.
    pub const fn backend_script_sha256(&self) -> [u8; 32] {
        self.script_hash
    }
    /// Borrow the retained installed distribution version; not publisher authentication.
    pub fn frontend_version(&self) -> &str {
        &self.frontend
    }
    /// Borrow the retained SugarCubes project version; not a remote version report.
    pub fn cube_version(&self) -> &str {
        &self.cube
    }
    /// Borrow every enrolled runtime asset, with source maps explicitly excluded.
    pub fn assets(&self) -> &[FrontendAsset] {
        &self.assets
    }
    /// Borrow the exact installed JavaScript URL inventory required in `/extensions`.
    pub fn sugarcubes_paths(&self) -> &[String] {
        &self.cube_paths
    }
}

/// Preserve an original installation boundary cause with redacted ordinary diagnostics.
pub struct FrontendSourceError {
    stage: &'static str,
    cause: Box<dyn Error + Send + Sync>,
}
impl FrontendSourceError {
    pub(crate) fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { stage, cause: Box::new(cause) }
    }
    pub(crate) fn invalid(stage: &'static str) -> Self {
        Self::new(stage, std::io::Error::other("Installed frontend boundary rejected"))
    }
    /// Borrow the stable operation label without private paths or metadata.
    pub const fn stage(&self) -> &'static str {
        self.stage
    }
}
impl fmt::Display for FrontendSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Installed frontend failure: {}", self.stage)
    }
}
impl fmt::Debug for FrontendSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl Error for FrontendSourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
