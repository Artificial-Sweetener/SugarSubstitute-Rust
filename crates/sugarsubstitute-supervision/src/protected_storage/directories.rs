// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Atomically return one directory handle under an actual retained protected parent.
use super::native_phase::NativePhase;
use super::*;
use crate::claim_error::BoundedNativeCause;
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
/// Specify one relative child under its retained actual parent.
pub(super) struct RelativeDirectory<'a> {
    pub(super) path: &'a Path,
    pub(super) parent: usize,
    pub(super) mode: Mode,
}
pub(super) fn open_relative(
    handles: &mut Vec<Handle>,
    sid: &[u8],
    budget: files::Budget,
    directory: RelativeDirectory<'_>,
    context: &JobContext,
    phase: &mut NativePhase<'_>,
) -> Result<usize, ClaimError> {
    let RelativeDirectory { path, parent, mode } = directory;
    phase.context(context)?;
    phase.check_budget(&budget)?;
    paths::validate_local_shape(path).map_err(|operation| phase.invalid(operation))?;
    if handles.len() == HANDLE_BOUND || handles.len() == handles.capacity() {
        return Err(phase.invalid("storage_handle_bound"));
    }
    let parent_raw =
        handles.get(parent).ok_or_else(|| phase.invalid("directory_parent_owner"))?.raw();
    let child = path.file_name().ok_or_else(|| phase.invalid("generation_child_name"))?;
    let mut producing = phase.relative_buffers(child, sid)?;
    let length = match u16::try_from(producing.units().len() * 2) {
        Ok(length) => length,
        Err(cause) => {
            drop(producing);
            return Err(phase.error("generation_child_bound", BoundedNativeCause::Integer(cause)));
        }
    };
    let object_name = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: PWSTR(producing.units().as_ptr().cast_mut()),
    };
    let attributes = match producing.attributes() {
        Some(attributes) => attributes,
        None => {
            drop(producing);
            return Err(phase.invalid("directory_descriptor_absent"));
        }
    };
    let object = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent_raw,
        ObjectName: &object_name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        SecurityDescriptor: attributes.lpSecurityDescriptor.cast(),
        ..Default::default()
    };
    let index = handles.len();
    handles.push(Handle::new(HANDLE::default(), budget.clone()));
    let mut raw = HANDLE::default();
    let mut io = IO_STATUS_BLOCK::default();
    producing.context(context)?;
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
    handles[index].adopt_output(raw);
    drop(producing);
    if status.0 != 0 && !(mode == Mode::OpenOrCreate && status.0 == 0x40000000) {
        // SAFETY: Pure conversion of this immediate native status preserves its error code.
        let code = unsafe { RtlNtStatusToDosError(status) };
        return Err(phase.error(
            "create_capture_generation",
            BoundedNativeCause::RawIo(std::io::Error::from_raw_os_error(code as i32)),
        ));
    }
    // FILE_CREATED=2 is the pinned SDK result; success never substitutes existing state.
    if raw.0.is_null() || !accepted(mode, status.0, io.Information) {
        return Err(phase.invalid("generation_creation_output"));
    }
    paths::validate_object(&handles[index], true, true, phase)?;
    let identity = paths::file_identity(&handles[index], phase)?;
    security::verify(&handles[index], sid, phase)?;
    let destination = phase.name_destination()?;
    let canonical = paths::final_path(&handles[index], phase, destination)?;
    if (mode == Mode::CreateNew && canonical.path()? != path)
        || (mode == Mode::OpenOrCreate && canonical.path()?.parent() != path.parent())
    {
        return Err(phase.invalid("capture_generation_resolution"));
    }
    paths::verify_pinned(canonical.path()?, identity, handles, true, budget, phase)?;
    phase.context(context)?;
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
