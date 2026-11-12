// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native disk-handle duplication and unique initial-entry adoption for installation processes.
use std::{
    fs::File,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle},
};
use windows::Win32::{
    Foundation::{
        DUPLICATE_HANDLE_OPTIONS, DUPLICATE_SAME_ACCESS, DuplicateHandle, GetHandleInformation,
        HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAG_PROTECT_FROM_CLOSE, HANDLE_FLAGS,
        SetHandleInformation, SetLastError, WIN32_ERROR,
    },
    Storage::FileSystem::{FILE_TYPE_DISK, GetFileType},
    System::{
        Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        Threading::GetCurrentProcess,
    },
};

/// Duplicate an already owned file for an explicit process-creation handle allowlist.
pub(in crate::setup_storage) fn duplicate_inheritable(file: &File) -> io::Result<File> {
    duplicate(file, 0, DUPLICATE_SAME_ACCESS)
}

/// Restrict a completed transient snapshot to read-only access before initial-entry transfer.
pub(in crate::setup_storage) fn duplicate_readonly_inheritable(file: &File) -> io::Result<File> {
    duplicate(
        file,
        windows::Win32::Storage::FileSystem::FILE_GENERIC_READ.0,
        DUPLICATE_HANDLE_OPTIONS(0),
    )
}

/// Keep unique native duplication and ownership adoption at one boundary.
fn duplicate(file: &File, access: u32, options: DUPLICATE_HANDLE_OPTIONS) -> io::Result<File> {
    let mut duplicate = HANDLE::default();
    // SAFETY: the retained file and current-process pseudo handle remain valid; the output
    // slot is uniquely owned and is adopted immediately after successful duplication.
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(file.as_raw_handle()),
            GetCurrentProcess(),
            &mut duplicate,
            access,
            true,
            options,
        )
    }
    .map_err(io::Error::other)?;
    // SAFETY: DuplicateHandle just returned this uniquely owned real file handle.
    Ok(unsafe { File::from_raw_handle(duplicate.0) })
}

/// Adopt one inherited disk handle once, narrowing inheritance before any descendants start.
/// # Safety
/// The initial-entry caller must guarantee unique ownership and prevent closure/reuse.
pub(in crate::setup_storage) unsafe fn capture_disk(raw: usize) -> io::Result<File> {
    if raw == 0 || raw == usize::MAX {
        return Err(super::invalid("Inherited disk handle is invalid"));
    }
    let handle = HANDLE(raw as *mut _);
    for standard in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: reset only this thread's last-error slot. GetStdHandle returns NULL without
        // setting an error when a GUI process has no standard stream.
        unsafe { SetLastError(WIN32_ERROR(0)) };
        // SAFETY: querying a process standard slot does not borrow or close its handle.
        let original = optional_standard(unsafe { GetStdHandle(standard) })?;
        if original == Some(handle) {
            return Err(super::invalid("Inherited disk object aliases a standard handle"));
        }
    }
    let mut flags = 0;
    // SAFETY: GetHandleInformation safely rejects invalid numeric inputs without dereferencing them.
    unsafe { GetHandleInformation(handle, &mut flags) }.map_err(io::Error::other)?;
    // SAFETY: this query observes the validated live handle and creates no ownership.
    let kind = unsafe { GetFileType(handle) };
    if flags & HANDLE_FLAG_INHERIT.0 == 0
        || flags & HANDLE_FLAG_PROTECT_FROM_CLOSE.0 != 0
        || kind != FILE_TYPE_DISK
    {
        return Err(super::invalid("Object is not an inherited disk handle"));
    }
    // SAFETY: the initial-entry caller guarantees no local owner exists. The validated handle
    // is inherited, real, non-standard and closeable; this is its sole adoption in the child.
    let file = unsafe { File::from_raw_handle(handle.0) };
    // SAFETY: file owns this handle. Prevent propagation into backend/desktop descendants.
    unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
        .map_err(io::Error::other)?;
    Ok(file)
}

/// The windows binding maps a valid missing standard stream (NULL, last-error zero) to Err.
fn optional_standard(value: windows::core::Result<HANDLE>) -> io::Result<Option<HANDLE>> {
    match value {
        Ok(handle) => Ok(Some(handle)),
        Err(cause) if cause.code().0 == 0 => Ok(None),
        Err(cause) => Err(io::Error::other(cause)),
    }
}

#[cfg(test)]
mod tests;
