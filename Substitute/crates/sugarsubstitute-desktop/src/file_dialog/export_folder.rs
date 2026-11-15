// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Choose the folder a save dialog opens at: the last folder the user saved an export to, else
//! the user's own Documents folder, else their profile folder.
//!
//! SugarSubstitute's processes may run with home, application-data and temporary folders
//! redirected into the installation, so the folders the environment names are not the user's.
//! A folder is offered only when it exists and lies outside the installation, outside every
//! redirected folder and outside any ComfyUI installation.
use std::path::{Path, PathBuf};

/// The candidates for one save dialog, in the order they are preferred.
#[derive(Debug, Default)]
pub(super) struct Candidates {
    /// The folder the dialog remembers from the user's last save of this kind.
    pub(super) remembered: Option<PathBuf>,
    /// The user's Documents folder candidates, best first.
    pub(super) documents: Vec<PathBuf>,
    /// The user's profile folder.
    pub(super) profile: Option<PathBuf>,
    /// Folders no candidate may lie inside: the installation and redirected folders.
    pub(super) excluded: Vec<PathBuf>,
}

impl Candidates {
    /// The first candidate that exists and belongs to the user, if any does.
    pub(super) fn choose(&self) -> Option<PathBuf> {
        let excluded: Vec<PathBuf> = self.excluded.iter().filter_map(|root| normal(root)).collect();
        self.remembered
            .iter()
            .chain(&self.documents)
            .chain(&self.profile)
            .filter_map(|candidate| normal(candidate))
            .find(|candidate| {
                !excluded.iter().any(|root| inside(candidate, root)) && !in_comfyui(candidate)
            })
    }
}

/// `path` when it names an existing absolute directory.
fn normal(path: &Path) -> Option<PathBuf> {
    (path.is_absolute() && path.is_dir()).then(|| path.to_path_buf())
}

/// Whether `path` is `root` or lies inside it, ignoring letter case as Windows does.
fn inside(path: &Path, root: &Path) -> bool {
    let fold = |path: &Path| -> Vec<String> {
        path.components().map(|part| part.as_os_str().to_string_lossy().to_lowercase()).collect()
    };
    fold(path).starts_with(&fold(root))
}

/// Whether `path` lies inside a ComfyUI installation, such as its workflows folder.
fn in_comfyui(path: &Path) -> bool {
    path.ancestors().any(|folder| folder.join("main.py").is_file() && folder.join("comfy").is_dir())
}

/// The installation holding the running executable: the outermost folder above it that holds
/// `SugarSubstitute.exe`, or the executable's own folder.
pub(super) fn installation() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let folder = executable.parent()?;
    folder
        .ancestors()
        .filter(|ancestor| ancestor.join("SugarSubstitute.exe").is_file())
        .last()
        .unwrap_or(folder)
        .to_path_buf()
        .into()
}

/// Folders the environment names for the user's home, settings and temporary files that lie
/// outside the user's real profile: they were redirected, so nothing in them is the user's.
pub(super) fn redirected(profile: Option<&Path>) -> Vec<PathBuf> {
    let profile = profile.and_then(normal);
    ["USERPROFILE", "HOME", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .filter(|folder| {
            let folder = normal(folder).unwrap_or_else(|| folder.clone());
            !profile.as_ref().is_some_and(|profile| inside(&folder, profile))
        })
        .collect()
}

/// Replace `%USERPROFILE%` in a raw shell-folder value with the real profile, and nothing else.
/// A value that still names another variable is not used.
pub(super) fn expand_profile(raw: &str, profile: &Path) -> Option<PathBuf> {
    const NAME: &str = "%userprofile%";
    let lower = raw.to_lowercase();
    let expanded = match lower.find(NAME) {
        Some(at) if lower.len() == raw.len() => {
            format!("{}{}{}", &raw[..at], profile.display(), &raw[at + NAME.len()..])
        }
        Some(_) => return None,
        None => raw.to_owned(),
    };
    (!expanded.contains('%')).then(|| PathBuf::from(expanded))
}

/// The folder a save dialog that remembers `remembered` opens at.
#[cfg(windows)]
pub(super) fn resolve(remembered: Option<PathBuf>) -> Option<PathBuf> {
    use windows::Win32::UI::Shell::{FOLDERID_Documents, FOLDERID_Profile};
    let profile = known_folder(&FOLDERID_Profile);
    let mut documents: Vec<PathBuf> = known_folder(&FOLDERID_Documents).into_iter().collect();
    if let Some(profile) = &profile {
        documents
            .extend(shell_folder_value("Personal").and_then(|raw| expand_profile(&raw, profile)));
    }
    let mut excluded = redirected(profile.as_deref());
    excluded.extend(installation());
    Candidates { remembered, documents, profile, excluded }.choose()
}

/// A known folder's path. The profile folder comes from the account itself; folders inside it
/// may follow the redirected environment, which the candidates' checks reject.
#[cfg(windows)]
fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };
    // SAFETY: The identifier outlives the call; the returned string is copied, then freed once.
    unsafe {
        let pointer = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let path = PathBuf::from(std::ffi::OsString::from_wide(pointer.as_wide()));
        CoTaskMemFree(Some(pointer.0.cast()));
        Some(path)
    }
}

/// The unexpanded value the account records for one of its shell folders.
#[cfg(windows)]
fn shell_folder_value(name: &str) -> Option<String> {
    use windows::{
        Win32::{
            Foundation::ERROR_SUCCESS,
            System::Registry::{
                HKEY_CURRENT_USER, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW,
            },
        },
        core::{HSTRING, w},
    };
    let name = HSTRING::from(name);
    let mut buffer = vec![0u16; 1024];
    let mut length = (buffer.len() * 2) as u32;
    // SAFETY: Read one bounded string value into an owned buffer without retaining registry state.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders"),
            &name,
            RRF_RT_REG_EXPAND_SZ | RRF_RT_REG_SZ | RRF_NOEXPAND,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut length),
        )
    };
    if result != ERROR_SUCCESS {
        return None;
    }
    buffer.truncate((length as usize / 2).min(buffer.len()));
    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    String::from_utf16(&buffer[..end]).ok()
}
#[cfg(test)]
#[path = "export_folder_tests.rs"]
mod tests;
