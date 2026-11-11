// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare expected installed metadata separately from managed process argv.
use super::{
    ExtensionInventory, MAX_EXTENSION_PATH_BYTES, MAX_EXTENSION_PATHS, ReadinessError, SystemStats,
    schema::bounded_text,
};
use std::{collections::BTreeSet, fmt};
/// Expected installation metadata awaiting retained native file identity validation.
/// This constructor accepts claimed metadata only. Versions and required URL paths
/// must be supplied by the installation owner, independently of response bytes.
pub struct InstalledFrontendIdentity {
    frontend_version: String,
    cube_version: String,
    required_paths: Vec<String>,
}
impl fmt::Debug for InstalledFrontendIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstalledFrontendIdentity")
            .field("required_paths", &self.required_paths.len())
            .finish_non_exhaustive()
    }
}
impl InstalledFrontendIdentity {
    /// Validate bounded nonempty versions and installed SugarCubes URL inventory.
    /// Performs no file reads, package discovery or native trust construction.
    pub fn new(
        frontend_version: String,
        cube_version: String,
        required_paths: Vec<String>,
    ) -> Result<Self, ReadinessError> {
        if !bounded_text(&frontend_version, 256)
            || !bounded_text(&cube_version, 256)
            || required_paths.is_empty()
        {
            return Err(ReadinessError::Shape);
        }
        validate_paths(&required_paths, true)?;
        Ok(Self { frontend_version, cube_version, required_paths })
    }
    /// Return the expected installed frontend version, not a verified filesystem fact.
    pub fn frontend_version(&self) -> &str {
        &self.frontend_version
    }
    /// Return the installed SugarCubes version claim. `/extensions` supplies paths
    /// only; response comparison cannot prove remote Cube version or asset bytes.
    pub fn cube_version(&self) -> &str {
        &self.cube_version
    }
    /// Borrow every expected installed SugarCubes URL required in the served inventory.
    pub fn required_paths(&self) -> &[String] {
        &self.required_paths
    }
    /// Check served frontend installed version and exact required path membership.
    /// This comparison supports both explicit attachment and managed launch.
    /// It does not check managed argv or authenticate a listener/process, frontend
    /// execution, asset bytes or SugarCubes version. HTTP 200 alone never passes it.
    pub fn compare(
        &self,
        stats: &SystemStats,
        extensions: &ExtensionInventory,
    ) -> Result<(), ReadinessError> {
        let package = stats.packages().iter().find(|p| p.name == "comfyui-frontend-package");
        if package.and_then(|p| p.installed.as_deref()) != Some(&self.frontend_version) {
            return Err(ReadinessError::FrontendMismatch);
        }
        let served: BTreeSet<&str> = extensions.paths.iter().map(String::as_str).collect();
        if self.required_paths.iter().any(|p| !served.contains(p.as_str())) {
            return Err(ReadinessError::MissingExtension);
        }
        Ok(())
    }
}
pub(super) fn validate_paths(paths: &[String], cube_only: bool) -> Result<(), ReadinessError> {
    if paths.len() > MAX_EXTENSION_PATHS {
        return Err(ReadinessError::Shape);
    }
    let mut seen = BTreeSet::new();
    let mut total = 0;
    for path in paths {
        total += path.len();
        let lower = path.to_ascii_lowercase();
        if !bounded_text(path, MAX_EXTENSION_PATH_BYTES)
            || !path.starts_with("/extensions/")
            || !path.ends_with(".js")
            || (cube_only && !path.starts_with("/extensions/SugarCubes/"))
            || path.contains(['\\', '?', '#'])
            || lower.contains("%2f")
            || lower.contains("%5c")
            || lower.contains("%2e")
            || path[1..]
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
            || !seen.insert(path.as_str())
        {
            return Err(ReadinessError::Shape);
        }
    }
    if total > super::MAX_EXTENSIONS_BYTES {
        return Err(ReadinessError::TooLarge);
    }
    Ok(())
}
