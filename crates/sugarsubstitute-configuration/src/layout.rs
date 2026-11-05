// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the self-contained installation layout below one user-chosen root.
//!
//! The root holds exactly one file, the stable `SugarSubstitute.exe` entry. Everything else
//! lives in named folders below it, so an installation never writes into account profile
//! folders. These are inert path names: storage owners open, pin and protect the actual
//! directories, and nothing here grants creation, deletion or launch authority.
use std::{
    io,
    path::{Path, PathBuf},
};

/// Stable entry users launch; it selects and starts the accepted application release.
pub const ENTRY: &str = "SugarSubstitute.exe";
/// Folder name of a default installation root.
pub const PRODUCT_FOLDER: &str = "SugarSubstitute";
/// Installation-owned state: ownership marker, selection, update trust and setup exclusion.
pub const STATE: &str = "state";
/// Immutable application generations, each carrying its own native and Python runtime files.
pub const RELEASES: &str = "releases";
/// User-authored data that every installation change preserves.
pub const USER: &str = "user";
/// Per-account runtime records: process claims, incident stores and operational history.
pub const RUNTIME: &str = "runtime";
/// Setup workspace: the installer's package cache and downloads.
pub const SETUP: &str = "setup";
/// Desktop host data owned by the installation, including the WebView profile.
pub const DESKTOP_DATA: &str = "desktop-data";

/// One installation root and the folders named below it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationLayout {
    root: PathBuf,
}
impl InstallationLayout {
    /// Name the layout of an explicit installation root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve the installation owning a running image's directory.
    ///
    /// A directory inside `releases` belongs to that folder's parent root. Any other directory,
    /// such as the root holding the stable entry or an uninstalled build output, is its own root,
    /// so every process keeps its records beside the image it runs from.
    pub fn for_image_directory(directory: &Path) -> Self {
        let root = directory
            .parent()
            .filter(|releases| {
                releases
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(RELEASES))
            })
            .and_then(Path::parent)
            .unwrap_or(directory);
        Self::new(root)
    }

    /// Borrow the installation root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// The stable entry executable at the root.
    pub fn entry(&self) -> PathBuf {
        self.root.join(ENTRY)
    }
    /// Installation-owned state folder.
    pub fn state(&self) -> PathBuf {
        self.root.join(STATE)
    }
    /// Application generations folder.
    pub fn releases(&self) -> PathBuf {
        self.root.join(RELEASES)
    }
    /// User data folder.
    pub fn user(&self) -> PathBuf {
        self.root.join(USER)
    }
    /// Base of per-account runtime records; storage owners add the `runtime` child themselves.
    pub fn runtime_base(&self) -> &Path {
        &self.root
    }
    /// Default setup workspace folder.
    pub fn setup(&self) -> PathBuf {
        self.root.join(SETUP)
    }
    /// WebView profile owned by the desktop host.
    pub fn webview_profile(&self) -> PathBuf {
        self.root.join(DESKTOP_DATA).join("profile")
    }
}

/// Suggest `<volume of the setup executable>\SugarSubstitute` as the installation root.
///
/// The suggestion keeps the installation on the volume the user downloaded setup to and never
/// consults account profile folders.
/// # Errors
/// Rejects a relative executable path, which has no volume to install on.
pub fn default_root(setup_executable: &Path) -> io::Result<PathBuf> {
    if !setup_executable.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Setup executable path must be absolute",
        ));
    }
    let volume = setup_executable.ancestors().last().unwrap_or(setup_executable);
    Ok(volume.join(PRODUCT_FOLDER))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_uses_the_setup_volume_and_never_account_folders() -> io::Result<()> {
        if cfg!(windows) {
            assert_eq!(
                default_root(Path::new(r"D:\Downloads\帳戶 café\SugarSubstitute-Setup.exe"))?,
                Path::new(r"D:\SugarSubstitute")
            );
            assert_eq!(
                default_root(Path::new(r"\\server\share\setup\SugarSubstitute-Setup.exe"))?,
                Path::new(r"\\server\share\SugarSubstitute")
            );
        } else {
            assert_eq!(
                default_root(Path::new("/downloads/SugarSubstitute-Setup"))?,
                Path::new("/SugarSubstitute")
            );
        }
        assert!(default_root(Path::new("SugarSubstitute-Setup.exe")).is_err());
        Ok(())
    }

    #[test]
    fn layout_keeps_one_entry_file_and_named_folders_below_the_root() {
        let root = PathBuf::from(if cfg!(windows) { r"E:\Sugar 糖" } else { "/Sugar 糖" });
        let layout = InstallationLayout::new(root.clone());
        assert_eq!(layout.entry(), root.join("SugarSubstitute.exe"));
        for (folder, expected) in [
            (layout.state(), "state"),
            (layout.releases(), "releases"),
            (layout.user(), "user"),
            (layout.setup(), "setup"),
        ] {
            assert_eq!(folder, root.join(expected));
        }
        assert_eq!(layout.webview_profile(), root.join("desktop-data").join("profile"));
        assert_eq!(layout.runtime_base(), root);
    }

    #[test]
    fn running_images_resolve_their_installation_root() {
        let root = PathBuf::from(if cfg!(windows) { r"E:\Sugar" } else { "/Sugar" });
        let generation = root.join("Releases").join("0123");
        assert_eq!(InstallationLayout::for_image_directory(&generation).root(), root);
        assert_eq!(InstallationLayout::for_image_directory(&root).root(), root);
        let build = root.join("target").join("debug");
        assert_eq!(InstallationLayout::for_image_directory(&build).root(), build);
    }
}
