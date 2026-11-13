// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire one exact regular file or empty directory under its retained parent, or rename one exact
//! object to a tombstone beside it.
use super::{
    files::{self, Directory},
    invalid,
};
use std::io;
use std::{
    fs::{File, OpenOptions},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
};
use windows::{
    Wdk::Storage::FileSystem::{FileRenameInformation, NtSetInformationFile},
    Win32::{
        Foundation::{HANDLE, RtlNtStatusToDosError},
        Storage::FileSystem::{
            DELETE, FILE_DISPOSITION_INFO, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO,
            FileDispositionInfo, SYNCHRONIZE, SetFileInformationByHandle,
        },
        System::IO::IO_STATUS_BLOCK,
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

/// Exclusively open one child of `parent` for renaming or deletion without following a link.
/// Any other open handle to that object makes this fail with a sharing violation.
/// # Errors
/// Preserves the native cause; rejects a name that is not one child of `parent`.
pub(crate) fn open_exclusive(parent: &Directory, leaf: &str) -> io::Result<File> {
    files::flat_leaf(leaf)?;
    OpenOptions::new()
        .access_mode((DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE).0)
        .share_mode(0)
        .custom_flags(0x0020_0000 | 0x0200_0000)
        .open(parent.path().join(leaf))
}

/// Rename the exact object `file` refers to as the absent sibling `leaf` in its own parent.
/// `file` comes from [`open_exclusive`]; the caller retains the parent's ancestry pins.
/// An existing sibling is never replaced. A directory with any open descendant stays in place.
/// # Errors
/// Preserves the native cause; rejects a name that is not one child component.
pub(crate) fn rename_within_parent(file: &File, leaf: &str) -> io::Result<()> {
    files::flat_leaf(leaf)?;
    let units: Vec<u16> = leaf.encode_utf16().collect();
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = offset + (units.len() + 1) * 2;
    let mut buffer = vec![0_usize; bytes.div_ceil(size_of::<usize>())];
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: The zeroed, usize-aligned owned buffer holds FILE_RENAME_INFO followed by the
    // complete UTF-16 name and its terminator, within `bytes`. A null root directory and a
    // simple name rename within the object's actual parent. `file` stays open through this
    // synchronous call, so the rename applies only to the verified object it refers to.
    let status = unsafe {
        let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*information).Anonymous.ReplaceIfExists = false;
        (*information).FileNameLength = (units.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            units.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            units.len(),
        );
        NtSetInformationFile(
            HANDLE(file.as_raw_handle()),
            &mut status_block,
            buffer.as_ptr().cast(),
            bytes as u32,
            FileRenameInformation,
        )
    };
    if status.is_ok() {
        Ok(())
    } else {
        // SAFETY: Pure status-code translation with no pointers.
        Err(io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(status) } as i32))
    }
}
