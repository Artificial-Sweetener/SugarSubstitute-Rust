// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire one exact regular file or empty directory under its retained parent.
use super::{
    files::{self, Directory},
    invalid,
};
use std::io;
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

#[cfg(test)]
mod tests;

/// Delete only the previously inspected regular file while its parent remains pinned.
pub(crate) fn file(parent: &Directory, leaf: &str, identity: (u32, u32, u32)) -> io::Result<()> {
    object(parent, leaf, identity, false)
}
/// Remove one empty previously inspected directory through its exact native identity.
pub(crate) fn directory(
    parent: &Directory,
    leaf: &str,
    identity: (u32, u32, u32),
) -> io::Result<()> {
    object(parent, leaf, identity, true)
}
/// Verify exclusive delete custody before marking the native object for deletion.
fn object(
    parent: &Directory,
    leaf: &str,
    identity: (u32, u32, u32),
    directory: bool,
) -> io::Result<()> {
    files::relative(leaf)?;
    if leaf.contains('/') {
        return Err(invalid("Retirement requires one child of its retained parent"));
    }
    let path = parent.path().join(leaf);
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
        return Err(invalid("Retirement target changed native identity"));
    }
    if !directory {
        files::single_link(&file)?;
    }
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: The exclusive handle owns the verified native object. The initialized ABI record
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
