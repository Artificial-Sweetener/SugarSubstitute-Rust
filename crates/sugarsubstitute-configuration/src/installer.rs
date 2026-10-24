// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installer location defaults resolved from the supported account environment input.
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

/// Inert installer locations. Native storage validates and retains selected directories
/// before use; these values grant no installation, deletion or update authority.
pub struct InstallerLocations {
    workspace: PathBuf,
    install_root: PathBuf,
}
impl InstallerLocations {
    /// Capture LOCALAPPDATA once. An explicit workspace wins over its default;
    /// the initial installation suggestion always uses account data.
    /// # Errors
    /// Preserves absence as the existing installer startup failure. Writes no settings.
    pub fn capture(workspace: Option<PathBuf>) -> io::Result<Self> {
        Self::resolve(workspace, std::env::var_os("LOCALAPPDATA"))
    }
    fn resolve(workspace: Option<PathBuf>, local: Option<OsString>) -> io::Result<Self> {
        let local = PathBuf::from(
            local.ok_or_else(|| io::Error::other("Local application data is unavailable"))?,
        );
        Ok(Self {
            workspace: workspace.unwrap_or_else(|| local.join("SugarSubstitute").join("setup")),
            install_root: local.join("Programs").join("SugarSubstitute"),
        })
    }
    /// Borrow the selected workspace without granting native storage authority.
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }
    /// Borrow the initial suggestion; saved maintenance selections take precedence.
    pub fn install_root(&self) -> &Path {
        &self.install_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_explicit_workspace_preserve_native_path_spelling() -> io::Result<()> {
        let root = PathBuf::from(if cfg!(windows) { r"E:\帳戶 café" } else { "/帳戶 café" });
        let defaults = InstallerLocations::resolve(None, Some(root.clone().into_os_string()))?;
        assert_eq!(defaults.workspace(), root.join("SugarSubstitute/setup"));
        assert_eq!(defaults.install_root(), root.join("Programs/SugarSubstitute"));
        let workspace = root.join("explicit 空間");
        let explicit =
            InstallerLocations::resolve(Some(workspace.clone()), Some(root.into_os_string()))?;
        assert_eq!(explicit.workspace(), workspace);
        assert_eq!(explicit.install_root(), defaults.install_root());
        Ok(())
    }
    #[test]
    fn explicit_workspace_does_not_hide_missing_account_location() {
        assert_eq!(
            InstallerLocations::resolve(Some(PathBuf::from("explicit")), None)
                .err()
                .map(|cause| cause.to_string())
                .as_deref(),
            Some("Local application data is unavailable")
        );
    }
}
