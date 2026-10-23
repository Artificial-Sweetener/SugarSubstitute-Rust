// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Mark only a verified native regular file for deletion while its parent remains pinned.
use super::*;
use std::{
    fs::OpenOptions,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
};
use windows::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{
        DELETE, FILE_DISPOSITION_INFO, FILE_READ_ATTRIBUTES, FileDispositionInfo, SYNCHRONIZE,
        SetFileInformationByHandle,
    },
};

/// Delete only the previously inspected regular file while its parent remains pinned.
pub(super) fn file(path: &Path, identity: (u32, u32, u32)) -> io::Result<()> {
    object(path, identity, false)
}
/// Remove one empty previously inspected directory through its exact native identity.
pub(super) fn directory(path: &Path, identity: (u32, u32, u32)) -> io::Result<()> {
    object(path, identity, true)
}
/// Verify exclusive delete custody before marking the native object for deletion.
fn object(path: &Path, identity: (u32, u32, u32), directory: bool) -> io::Result<()> {
    let file = OpenOptions::new()
        .access_mode((DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE).0)
        .share_mode(0)
        .custom_flags(0x0020_0000 | if directory { 0x0200_0000 } else { 0 })
        .open(path)?;
    let metadata = file.metadata()?;
    if (if directory { !metadata.is_dir() } else { !metadata.is_file() })
        || metadata.file_attributes() & 0x400 != 0
        || files::file_identity(&file)? != identity
    {
        return Err(invalid("Cache deletion target changed native identity"));
    }
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: The exclusive handle owns the verified regular file. The initialized ABI record
    // and its exact byte size remain live through this synchronous call. Parent pins prevent
    // ancestor replacement. Closing this sole handle completes deletion of only this object.
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            (&disposition as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&disposition) as u32,
        )
    }
    .map_err(io::Error::other)
}

/// Reject multiple directory links before any in-place write to cache-owned scratch storage.
pub(super) fn single_link(file: &File) -> io::Result<()> {
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the borrowed file keeps this valid handle live for the synchronous query and
    // the initialized output has exactly the Windows ABI layout expected by the function.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information) }
        .map_err(io::Error::other)?;
    if information.nNumberOfLinks != 1 {
        return Err(invalid("Cache writable files must have exactly one directory link"));
    }
    Ok(())
}
