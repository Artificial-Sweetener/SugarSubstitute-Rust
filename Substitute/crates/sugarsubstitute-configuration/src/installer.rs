// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installer location defaults resolved from the setup executable's own location.
use crate::layout::{self, InstallationLayout};
use std::{
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
    /// Suggest `<setup volume>\SugarSubstitute` and keep the setup workspace in its `setup` folder.
    /// An explicit workspace wins over that default. Account profile folders are never consulted.
    /// # Errors
    /// Rejects a relative setup executable path. Writes no settings.
    pub fn resolve(workspace: Option<PathBuf>, setup_executable: &Path) -> io::Result<Self> {
        let install_root = layout::default_root(setup_executable)?;
        Ok(Self {
            workspace: workspace.unwrap_or_else(|| InstallationLayout::new(&install_root).setup()),
            install_root,
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
    fn defaults_follow_the_setup_volume_and_preserve_native_path_spelling() -> io::Result<()> {
        let volume = PathBuf::from(if cfg!(windows) { r"E:\" } else { "/" });
        let setup = volume.join("下載 café").join("SugarSubstitute-Setup.exe");
        let defaults = InstallerLocations::resolve(None, &setup)?;
        assert_eq!(defaults.install_root(), volume.join("SugarSubstitute"));
        assert_eq!(defaults.workspace(), volume.join("SugarSubstitute").join("setup"));
        let workspace = volume.join("explicit 空間");
        let explicit = InstallerLocations::resolve(Some(workspace.clone()), &setup)?;
        assert_eq!(explicit.workspace(), workspace);
        assert_eq!(explicit.install_root(), defaults.install_root());
        Ok(())
    }
    #[test]
    fn defaults_never_resolve_into_account_profile_folders() -> io::Result<()> {
        let volume = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
        let setup =
            volume.join("Users").join("someone").join("AppData").join("Local").join("s.exe");
        let defaults = InstallerLocations::resolve(None, &setup)?;
        for path in [defaults.install_root(), defaults.workspace()] {
            assert!(
                !path.components().any(|part| part.as_os_str().eq_ignore_ascii_case("AppData")),
                "{path:?}"
            );
        }
        Ok(())
    }
    #[test]
    fn relative_setup_location_is_rejected() {
        assert!(InstallerLocations::resolve(None, Path::new("setup.exe")).is_err());
    }
}
