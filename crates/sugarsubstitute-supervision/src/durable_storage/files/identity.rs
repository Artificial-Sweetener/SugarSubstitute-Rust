// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare pinned directories by actual filesystem identity rather than caller path spelling.
use crate::durable_storage::files::{Directory, invalid};
use std::{fs::File, io, os::windows::io::AsRawHandle};
use windows::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO, FileIdInfo, GetFileInformationByHandle,
        GetFileInformationByHandleEx,
    },
};
impl Directory {
    /// Observe the same volume/128-bit file identity used by persisted native profile rows.
    pub(crate) fn native_identity(&self) -> io::Result<[u8; 24]> {
        let file = self._pins.last().ok_or_else(|| invalid("Directory handle missing"))?;
        native_identity(file)
    }
    /// Query the retained leaf; ancestor pins prevent path replacement during the query.
    pub(crate) fn identity(&self) -> io::Result<(u32, u32, u32)> {
        let file = self._pins.last().ok_or_else(|| invalid("Directory handle missing"))?;
        file_identity(file)
    }
}

/// Observe full volume/128-bit identity from an already retained file or directory.
/// The caller independently verifies ancestry, kind, protection and link-count policy.
pub(crate) fn native_identity(file: &File) -> io::Result<[u8; 24]> {
    let mut information = FILE_ID_INFO::default();
    // SAFETY: The borrowed live file and exact initialized ABI output stay retained.
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileIdInfo,
            std::ptr::from_mut(&mut information).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    }
    .map_err(io::Error::other)?;
    let mut identity = [0; 24];
    identity[..8].copy_from_slice(&information.VolumeSerialNumber.to_le_bytes());
    identity[8..].copy_from_slice(&information.FileId.Identifier);
    Ok(identity)
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
