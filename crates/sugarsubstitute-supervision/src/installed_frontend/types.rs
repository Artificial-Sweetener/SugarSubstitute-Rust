// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound claimed installation locations and immutable native inspection observations.
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::execution::{JobCompletion, JobOutcome, PanicCause, TerminalKind};

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
    /// Compare exact spellings of all four independently selected source paths.
    /// This borrows only `OsStr` values; it performs no filesystem access,
    /// canonicalization, case folding or source inspection and grants no trust.
    pub fn same_selection(&self, other: &Self) -> bool {
        self.package.as_os_str() == other.package.as_os_str()
            && self.metadata.as_os_str() == other.metadata.as_os_str()
            && self.cube.as_os_str() == other.cube.as_os_str()
            && self.script.as_os_str() == other.script.as_os_str()
    }
    /// Reissue the same explicit inert selection for another finite inspection.
    /// This copies no proof or native owner. The composing parent retains its
    /// preclaimed input-copy partition until the new inspector takes custody.
    /// # Errors
    /// Preserves allocation causes and rejects unexpected capacity growth.
    pub fn try_reissue(&self) -> Result<Self, FrontendSourceError> {
        fn copy(path: &Path) -> Result<PathBuf, FrontendSourceError> {
            let mut result = PathBuf::new();
            result
                .try_reserve_exact(path.as_os_str().len())
                .map_err(|cause| FrontendSourceError::new("source_reissue_allocation", cause))?;
            result.push(path);
            if result.capacity() > 128 * 1024 {
                return Err(FrontendSourceError::invalid("source_reissue_capacity"));
            }
            Ok(result)
        }
        Self::new(
            copy(&self.package)?,
            copy(&self.metadata)?,
            copy(&self.cube)?,
            copy(&self.script)?,
        )
    }
    /// Borrow the selected package directory; this is not a canonical proof.
    pub fn package_directory(&self) -> &Path {
        &self.package
    }
}

/// One inspected runtime file and its immutable served URL/hash/length snapshot.
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
    /// Return SHA-256 observed under inspection pins; later disk changes do not alter it.
    pub const fn sha256(&self) -> [u8; 32] {
        self.hash
    }
}
impl fmt::Debug for FrontendAsset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrontendAsset").field("bytes", &self.bytes).finish_non_exhaustive()
    }
}

/// Immutable inspection observations held by an active snapshot lease.
/// Native inspection handles are closed before publication; current disk content may change.
pub struct FrontendSourceManifest {
    pub(crate) frontend: String,
    pub(crate) cube: String,
    pub(crate) assets: Vec<FrontendAsset>,
    pub(crate) cube_paths: Vec<String>,
    pub(crate) script: PathBuf,
    pub(crate) script_hash: [u8; 32],
}
impl FrontendSourceManifest {
    /// Borrow the backend script path canonicalized during completed inspection.
    pub fn backend_script(&self) -> &Path {
        &self.script
    }
    /// Return SHA-256 of the complete backend script observed during inspection.
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
#[derive(Clone)]
pub struct FrontendSourceError {
    stage: &'static str,
    cause: Arc<dyn Error + Send + Sync>,
}
impl FrontendSourceError {
    pub(crate) fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { stage, cause: Arc::new(cause) }
    }
    pub(crate) fn invalid(stage: &'static str) -> Self {
        Self::new(stage, std::io::Error::other("Installed frontend boundary rejected"))
    }
    /// Retain the genuine completion, including panic and byte custody, before returning its marker.
    pub(crate) fn terminal(
        completion: JobCompletion<(), FrontendSourceError>,
        matched: bool,
    ) -> Self {
        Self::new(
            if matched { "source_terminal" } else { "source_delivery_identity" },
            InspectionTerminal { completion, matched },
        )
    }
    /// Borrow actual execution terminal metadata only after original delivery identity matched.
    /// Domain failures and rejected foreign completions return `None`; this grants no native closure.
    pub fn terminal_kind(&self) -> Option<TerminalKind> {
        let original = self.cause.downcast_ref::<InspectionTerminal>()?;
        original.matched.then_some(original.completion.outcome.kind())
    }
    /// Borrow the original bounded panic payload without formatting or logging it.
    pub fn panic_cause(&self) -> Option<&PanicCause> {
        match &self.cause.downcast_ref::<InspectionTerminal>()?.completion.outcome {
            JobOutcome::Panicked(cause) => Some(cause),
            _ => None,
        }
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

/// Original typed terminal remains owned independently of partial file inspection and explicit closure.
struct InspectionTerminal {
    completion: JobCompletion<(), FrontendSourceError>,
    matched: bool,
}
impl fmt::Debug for InspectionTerminal {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_struct("InspectionTerminal")
            .field("kind", &self.completion.outcome.kind())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for InspectionTerminal {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "Installed inspection terminal: {:?}", self.completion.outcome.kind())
    }
}
impl Error for InspectionTerminal {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.completion.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
}
