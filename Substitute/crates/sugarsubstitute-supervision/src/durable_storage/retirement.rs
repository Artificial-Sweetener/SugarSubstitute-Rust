// SPDX-License-Identifier: AGPL-3.0-or-later
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
            DELETE, FILE_BASIC_INFO, FILE_DISPOSITION_INFO, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO,
            FILE_WRITE_ATTRIBUTES, FileBasicInfo, FileDispositionInfo, SYNCHRONIZE,
            SetFileInformationByHandle,
        },
        System::IO::IO_STATUS_BLOCK,
    },
};

/// `ERROR_ACCESS_DENIED`.
const ACCESS_DENIED: i32 = 5;
/// `FILE_ATTRIBUTE_READONLY`.
const READ_ONLY: u32 = 0x1;
/// `FILE_ATTRIBUTE_NORMAL`.
const NORMAL: u32 = 0x80;

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
    let open = |access: u32| {
        OpenOptions::new()
            .access_mode(access)
            .share_mode(0)
            .custom_flags(0x0020_0000 | if directory { 0x0200_0000 } else { 0 })
            .open(&path)
    };
    let access = (DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE).0;
    // The right to change attributes is asked for only to clear a read-only attribute below.
    // An object that refuses that right is still deleted when it is not read-only.
    let file = match open(access | FILE_WRITE_ATTRIBUTES.0) {
        Err(cause) if cause.raw_os_error() == Some(ACCESS_DENIED) => open(access)?,
        opened => opened?,
    };
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
    // Windows refuses to delete a read-only object. This is the owner's own verified object,
    // so the attribute is cleared on this exact handle first. A handle without the right to
    // change attributes fails here with access denied, which callers retry like a held file.
    let attributes = metadata.file_attributes();
    if attributes & READ_ONLY != 0 {
        let cleared = attributes & !READ_ONLY;
        // Zero timestamps leave the object's times as they are; an object left with no
        // attribute at all is a normal one.
        let basic = FILE_BASIC_INFO {
            FileAttributes: if cleared == 0 { NORMAL } else { cleared },
            ..Default::default()
        };
        // SAFETY: The exclusive handle owns the verified native object. The initialized ABI
        // record and its exact byte size remain live through this synchronous call.
        unsafe {
            SetFileInformationByHandle(
                HANDLE(file.as_raw_handle()),
                FileBasicInfo,
                (&basic as *const FILE_BASIC_INFO).cast(),
                std::mem::size_of_val(&basic) as u32,
            )
        }
        .map_err(native)?;
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
    .map_err(native)
}

/// Keep a native failure's Win32 code, so a refusal such as access denied is told from other
/// failures. An HRESULT that wraps a Win32 error carries it in its low word.
fn native(cause: windows::core::Error) -> io::Error {
    let code = cause.code().0;
    if code as u32 & 0xFFFF_0000 == 0x8007_0000 {
        io::Error::from_raw_os_error(code & 0xFFFF)
    } else {
        io::Error::other(cause)
    }
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
