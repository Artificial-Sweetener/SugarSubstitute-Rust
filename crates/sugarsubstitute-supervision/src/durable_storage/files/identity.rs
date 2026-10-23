// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare pinned directories by actual filesystem identity rather than caller path spelling.
use crate::durable_storage::files::{Directory, invalid};
use std::{fs::File, io, os::windows::io::AsRawHandle};
use windows::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
};
impl Directory {
    /// Query the retained leaf; ancestor pins prevent path replacement during the query.
    pub(crate) fn identity(&self) -> io::Result<(u32, u32, u32)> {
        let file = self._pins.last().ok_or_else(|| invalid("Directory handle missing"))?;
        file_identity(file)
    }
}

/// Compare retained native objects without reopening a caller-controlled path.
pub(crate) fn file_identity(file: &File) -> io::Result<(u32, u32, u32)> {
    let information = information(file)?;
    Ok((information.dwVolumeSerialNumber, information.nFileIndexHigh, information.nFileIndexLow))
}

/// Reject multiple directory links before any in-place write to cache-owned scratch storage.
pub(crate) fn single_link(file: &File) -> io::Result<()> {
    let information = information(file)?;
    if information.nNumberOfLinks != 1 {
        return Err(invalid("Cache writable files must have exactly one directory link"));
    }
    Ok(())
}

/// Query one borrowed native handle for identity and link-count invariants before mutation.
fn information(file: &File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: File retains the valid handle and the initialized ABI output lives through the call.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information) }
        .map_err(io::Error::other)?;
    Ok(information)
}
