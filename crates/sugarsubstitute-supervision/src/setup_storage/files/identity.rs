// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare pinned directories by actual filesystem identity rather than caller path spelling.
use super::{Directory, invalid};
use std::{fs::File, io, os::windows::io::AsRawHandle};
use windows::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
};
impl Directory {
    /// Query the retained leaf; ancestor pins prevent path replacement during the query.
    pub(in crate::setup_storage) fn identity(&self) -> io::Result<(u32, u32, u32)> {
        let file = self._pins.last().ok_or_else(|| invalid("Directory handle missing"))?;
        file_identity(file)
    }
}

/// Compare retained native objects without reopening a caller-controlled path.
pub(in crate::setup_storage) fn file_identity(file: &File) -> io::Result<(u32, u32, u32)> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: File retains this valid handle and the ABI output remains live for this call.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information) }
        .map_err(io::Error::other)?;
    Ok((information.dwVolumeSerialNumber, information.nFileIndexHigh, information.nFileIndexLow))
}
