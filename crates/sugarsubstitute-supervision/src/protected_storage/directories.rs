// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Atomically return one directory handle under an actual retained protected parent.
use super::*;
use std::os::windows::ffi::OsStrExt;
use windows::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_CREATE, FILE_DIRECTORY_FILE, FILE_OPEN_IF, FILE_SYNCHRONOUS_IO_NONALERT,
            NtCreateFile,
        },
    },
    Win32::{
        Foundation::{
            HANDLE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, RtlNtStatusToDosError, UNICODE_STRING,
        },
        Storage::FileSystem::{
            FILE_ATTRIBUTE_DIRECTORY, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
            FILE_SHARE_WRITE, FILE_TRAVERSE, READ_CONTROL, SYNCHRONIZE,
        },
        System::IO::IO_STATUS_BLOCK,
    },
    core::PWSTR,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mode {
    CreateNew,
    OpenOrCreate,
}
fn accepted(mode: Mode, status: i32, information: usize) -> bool {
    match mode {
        Mode::CreateNew => status == 0 && information == 2,
        Mode::OpenOrCreate => {
            (status == 0 && matches!(information, 1 | 2))
                || (status == 0x40000000 && information == 1)
        }
    }
}
pub(super) fn open_relative(
    state: &mut State,
    path: &Path,
    parent: usize,
    mode: Mode,
    context: &JobContext,
) -> Result<usize, ClaimError> {
    check(context)?;
    paths::wide(path)?;
    if state.handles.len() == HANDLE_BOUND {
        return Err(ClaimError::invalid("storage_handle_bound"));
    }
    let child = path.file_name().ok_or_else(|| ClaimError::invalid("generation_child_name"))?;
    let mut units: Vec<u16> = child.encode_wide().collect();
    if units.is_empty() || units.iter().any(|unit| matches!(*unit, 0 | 47 | 58 | 92)) {
        return Err(ClaimError::invalid("directory_leaf_name"));
    }
    let length =
        u16::try_from(units.len() * 2).map_err(|e| ClaimError::new("generation_child_bound", e))?;
    let object_name =
        UNICODE_STRING { Length: length, MaximumLength: length, Buffer: PWSTR(units.as_mut_ptr()) };
    let descriptor = security::Descriptor::new(&state.sid)?;
    let attributes = descriptor.attributes();
    let object = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: state
            .handles
            .get(parent)
            .ok_or_else(|| ClaimError::invalid("directory_parent_owner"))?
            .raw(),
        ObjectName: &object_name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        SecurityDescriptor: attributes.lpSecurityDescriptor.cast(),
        ..Default::default()
    };
    let mut raw = HANDLE::default();
    let mut io = IO_STATUS_BLOCK::default();
    // SAFETY: Fixed one-component relative name, actual retained parent and aligned
    // descriptor remain live. Synchronous FILE_CREATE atomically returns the created
    // directory handle; an existing name fails. No handle inherits or shares deletion.
    let status = unsafe {
        NtCreateFile(
            &mut raw,
            FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY | FILE_TRAVERSE | READ_CONTROL | SYNCHRONIZE,
            &object,
            &mut io,
            None,
            FILE_ATTRIBUTE_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            if mode == Mode::CreateNew { FILE_CREATE } else { FILE_OPEN_IF },
            FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            None,
            0,
        )
    };
    let budget = files::Budget::Protected(state.memory.clone());
    let index = state.handles.len();
    if !raw.0.is_null() {
        state.handles.push(Handle::new(raw, budget));
    }
    if status.0 != 0 && !(mode == Mode::OpenOrCreate && status.0 == 0x40000000) {
        // SAFETY: Pure conversion of this immediate native status preserves its error code.
        let code = unsafe { RtlNtStatusToDosError(status) };
        return Err(ClaimError::new(
            "create_capture_generation",
            std::io::Error::from_raw_os_error(code as i32),
        ));
    }
    // FILE_CREATED=2 is the pinned SDK result; success never substitutes existing state.
    if raw.0.is_null() || !accepted(mode, status.0, io.Information) {
        return Err(ClaimError::invalid("generation_creation_output"));
    }
    paths::validate_object(&state.handles[index], true, true)?;
    let identity = paths::file_identity(&state.handles[index])?;
    security::verify(&state.handles[index], &state.sid)?;
    let canonical = paths::final_path(&state.handles[index])?;
    if (mode == Mode::CreateNew && canonical != path)
        || (mode == Mode::OpenOrCreate && canonical.parent() != path.parent())
    {
        return Err(ClaimError::invalid("capture_generation_resolution"));
    }
    paths::verify_pinned(
        &canonical,
        identity,
        &mut state.handles,
        true,
        files::Budget::Protected(state.memory.clone()),
    )?;
    check(context)?;
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_disposition_never_overwrites_or_admits_unknown_native_outputs() {
        assert!(accepted(Mode::CreateNew, 0, 2));
        assert!(!accepted(Mode::CreateNew, 0, 1));
        assert!(!accepted(Mode::CreateNew, 0x40000000, 1));
        for information in [1, 2] {
            assert!(accepted(Mode::OpenOrCreate, 0, information));
        }
        assert!(accepted(Mode::OpenOrCreate, 0x40000000, 1));
        assert!(!accepted(Mode::OpenOrCreate, 0x40000000, 2));
        for mode in [Mode::CreateNew, Mode::OpenOrCreate] {
            for information in [0, 3, 4, 5, usize::MAX] {
                assert!(!accepted(mode, 0, information));
            }
            for status in [-1, 0x40000001, 0x103] {
                assert!(!accepted(mode, status, 1));
            }
        }
    }
}
