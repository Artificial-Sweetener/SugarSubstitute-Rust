// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt actual native outputs before status checks and pin their canonical namespace.

use crate::{ClaimError, native_claims::files::Handle};
use std::{io::Write, os::windows::ffi::OsStrExt, path::Path};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Globalization::{CSTR_EQUAL, CompareStringOrdinal},
        Storage::FileSystem::*,
    },
    core::PCWSTR,
};

pub(super) const SCRATCH_BYTES: usize = 16 * 1024;

struct NativePath {
    units: [u16; 1030],
    length: usize,
}
impl NativePath {
    /// Bound lexical units and use an explicit local extended prefix to avoid alias trimming.
    fn new(path: &Path) -> Result<Self, ClaimError> {
        let verbatim = path.as_os_str().encode_wide().take(4).eq([92, 92, 63, 92]);
        let mut result = Self { units: [0; 1030], length: 0 };
        if !verbatim {
            result.units[..4].copy_from_slice(&[92, 92, 63, 92]);
            result.length = 4;
        }
        for unit in path.as_os_str().encode_wide() {
            if result.length == result.units.len() - 1 || unit == 0 {
                return Err(ClaimError::invalid("preview_artifact_native_path"));
            }
            result.units[result.length] =
                if unit == u16::from(b'/') { u16::from(b'\\') } else { unit };
            result.length += 1;
        }
        Ok(result)
    }
    fn pointer(&self) -> PCWSTR {
        PCWSTR(self.units.as_ptr())
    }
}

/// Open a pinned read-only existing directory, denying write/delete sharing of its own object.
pub(super) fn open_directory(path: &Path, output: &mut Handle) -> Result<(), ClaimError> {
    let path = NativePath::new(path)?;
    // SAFETY: The bounded NUL name lives throughout this noninheritable existing-only open.
    // Directory write/delete sharing is denied, and reparse points are opened as objects.
    let created = unsafe {
        CreateFileW(
            path.pointer(),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    };
    adopt(created, output)
}

/// Create the one output exclusively; an existing name can never be overwritten or adopted.
pub(super) fn create_png(path: &Path, output: &mut Handle) -> Result<(), ClaimError> {
    let path = NativePath::new(path)?;
    // SAFETY: The bounded NUL name is borrowed for this noninheritable CREATE_NEW effect;
    // the caller retains pinned ancestry and the returned output cell before validation.
    let created = unsafe {
        CreateFileW(
            path.pointer(),
            FILE_GENERIC_WRITE.0 | FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ,
            None,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    };
    adopt(created, output)
}

fn adopt(result: windows::core::Result<HANDLE>, output: &mut Handle) -> Result<(), ClaimError> {
    match result {
        Ok(handle) => {
            output.adopt_output(handle);
            Ok(())
        }
        Err(cause) => Err(ClaimError::new("open_preview_artifact", cause)),
    }
}

pub(super) fn validate_directory(output: &mut Handle, expected: &Path) -> Result<(), ClaimError> {
    validate(output, expected, true)
}
pub(super) fn validate_output(output: &mut Handle, expected: &Path) -> Result<(), ClaimError> {
    validate(output, expected, false)
}

/// Query the same retained object and reject reparse/volume/name/type mutation.
fn validate(output: &Handle, expected: &Path, directory: bool) -> Result<(), ClaimError> {
    let expected = NativePath::new(expected)?;
    let mut facts = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: The exact actual output is retained; Windows writes fixed stack metadata.
    unsafe { GetFileInformationByHandle(output.raw(), &mut facts) }
        .map_err(|cause| ClaimError::new("inspect_preview_artifact", cause))?;
    if facts.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        || (facts.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0) != directory
    {
        return Err(ClaimError::invalid("preview_artifact_object_type"));
    }
    let mut name = [0_u16; 1030];
    // SAFETY: The handle is still owned and Windows receives this exact stack output;
    // zero/oversized results are rejected before slicing or namespace comparison.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            output.raw(),
            &mut name,
            GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
        )
    };
    if length == 0 {
        return Err(ClaimError::new(
            "resolve_preview_artifact",
            windows::core::Error::from_thread(),
        ));
    }
    if length as usize >= name.len() || name[length as usize] != 0 {
        return Err(ClaimError::invalid("preview_artifact_resolved_path_bound"));
    }
    // SAFETY: Both explicit-length UTF-16 inputs are bounded stack storage. The OS ordinal
    // case comparison handles filesystem spelling without locale-dependent allocation.
    let equal = unsafe {
        CompareStringOrdinal(&name[..length as usize], &expected.units[..expected.length], true)
    };
    if equal != CSTR_EQUAL {
        return Err(ClaimError::invalid("preview_artifact_resolved_namespace"));
    }
    Ok(())
}

/// Borrow the retained physical file for a bounded write; borrow Drop restores the same handle.
pub(super) fn write(output: &mut Handle, bytes: &[u8]) -> Result<(), ClaimError> {
    let borrowed = output.borrow_file()?;
    let mut writer = &*borrowed;
    writer.write_all(bytes).map_err(|cause| ClaimError::new("write_preview_artifact", cause))
}

/// Check actual native flush while retaining output ownership independently of the result.
pub(super) fn flush(output: &mut Handle) -> Result<(), ClaimError> {
    // SAFETY: The exact owned file has completed authored writes; flush creates no new output.
    unsafe { FlushFileBuffers(output.raw()) }
        .map_err(|cause| ClaimError::new("flush_preview_artifact", cause))
}
