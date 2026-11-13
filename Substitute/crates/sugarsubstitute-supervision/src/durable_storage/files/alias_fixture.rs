// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reach owned test directories through real Windows path aliases without changing their objects.
//! A short-name spelling comes from the volume itself; a drive alias is a per-session DOS device
//! that this fixture defines on a free letter and removes again when dropped.
use super::invalid;
use std::{
    ffi::OsString,
    io,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};
use windows::{
    Win32::Storage::FileSystem::{
        DDD_NO_BROADCAST_SYSTEM, DDD_REMOVE_DEFINITION, DefineDosDeviceW, GetLogicalDrives,
        GetShortPathNameW,
    },
    core::HSTRING,
};

/// Spell an existing path with the 8.3 short name of every component that has one.
/// The result equals `path` on a volume that does not generate short names.
pub(crate) fn short_spelling(path: &Path) -> io::Result<PathBuf> {
    let long = HSTRING::from(path);
    // SAFETY: `long` is a live null-terminated UTF-16 string; no output buffer is passed.
    let required = unsafe { GetShortPathNameW(&long, None) };
    if required == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0_u16; required as usize];
    // SAFETY: `long` and the exclusively borrowed buffer of the reported length stay live.
    let written = unsafe { GetShortPathNameW(&long, Some(&mut buffer)) } as usize;
    if written == 0 {
        return Err(io::Error::last_os_error());
    }
    if written >= buffer.len() {
        return Err(invalid("Short path changed length between queries"));
    }
    Ok(PathBuf::from(OsString::from_wide(&buffer[..written])))
}

/// A drive letter defined for this logon session that redirects to one owned directory.
/// Call `remove` to observe removal; dropping an unremoved alias removes it as a fallback.
pub(crate) struct DriveAlias {
    device: HSTRING,
    root: PathBuf,
    defined: bool,
}

impl DriveAlias {
    /// Define the highest unused drive letter as an alias of `target`, as `subst` does.
    /// # Errors
    /// Fails when every letter is in use or the definition is refused.
    pub(crate) fn define(target: &Path) -> io::Result<Self> {
        // SAFETY: The call takes no arguments and only reads the session's drive bitmask.
        let used = unsafe { GetLogicalDrives() };
        let letter = (b'D'..=b'Z')
            .rev()
            .find(|letter| used & (1 << (letter - b'A')) == 0)
            .ok_or_else(|| invalid("No unused drive letter for a drive alias"))?;
        let device = HSTRING::from(format!("{}:", char::from(letter)));
        // SAFETY: Both null-terminated strings stay live for the synchronous call.
        unsafe { DefineDosDeviceW(DDD_NO_BROADCAST_SYSTEM, &device, &HSTRING::from(target)) }
            .map_err(io::Error::other)?;
        Ok(Self {
            device,
            root: PathBuf::from(format!("{}:\\", char::from(letter))),
            defined: true,
        })
    }

    /// The alias drive's root directory, which names the aliased target.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Remove this definition, as `subst /d` does, and report the native outcome.
    /// # Errors
    /// Preserves the native refusal; the letter then stays defined until logoff.
    pub(crate) fn remove(mut self) -> io::Result<()> {
        self.defined = false;
        undefine(&self.device)
    }
}

impl Drop for DriveAlias {
    fn drop(&mut self) {
        if self.defined
            && let Err(cause) = undefine(&self.device)
        {
            tracing::error!(target: "sugarsubstitute::storage", %cause, "drive_alias_remove");
        }
    }
}

/// Remove the most recent definition of `device`; the letter was unused before this fixture.
fn undefine(device: &HSTRING) -> io::Result<()> {
    // SAFETY: The null-terminated device string stays live; a null target selects the latest
    // definition, which is this fixture's own.
    unsafe {
        DefineDosDeviceW(
            DDD_REMOVE_DEFINITION | DDD_NO_BROADCAST_SYSTEM,
            device,
            windows::core::PCWSTR::null(),
        )
    }
    .map_err(io::Error::other)
}
