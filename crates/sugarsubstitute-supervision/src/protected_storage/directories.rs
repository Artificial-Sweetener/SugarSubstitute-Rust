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
            FILE_CREATE, FILE_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF,
            FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
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
pub(crate) enum Mode {
    CreateNew,
    OpenOrCreate,
    Existing,
}
/// Preserve the checked native creation disposition of this exact opened directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectoryDisposition {
    Created,
    Existing,
}
/// Retain the opened native slot and its checked full file identity.
pub(crate) struct OpenedDirectory {
    pub(crate) index: usize,
    pub(super) identity: [u8; 24],
    pub(super) disposition: DirectoryDisposition,
}
fn disposition(mode: Mode, status: i32, information: usize) -> Option<DirectoryDisposition> {
    match (mode, status, information) {
        (Mode::Existing, 0, 1) => Some(DirectoryDisposition::Existing),
        (Mode::Existing, _, _) => None,
        (_, 0, 2) => Some(DirectoryDisposition::Created),
        (Mode::OpenOrCreate, 0 | 0x40000000, 1) => Some(DirectoryDisposition::Existing),
        _ => None,
    }
}
/// Check one actual canonical verification close while preserving the caller's physical slot.
pub(crate) type VerificationClose =
    for<'a> fn(&mut Handle, &mut NativePhase<'a>) -> Result<(), ClaimError>;
/// Own both counted outputs before the first relative native call.
pub(crate) struct CountedDirectoryOutputs {
    /// Permanent same-parent child pin.
    pub(crate) primary: Handle,
    /// Distinct canonical reopen retained in the vector until checked closure.
    pub(crate) verification: Handle,
    /// Native close boundary evaluated under the same original running phase.
    pub(crate) close_verification: VerificationClose,
}
/// Specify one relative child under its retained actual parent.
pub(crate) struct RelativeDirectory<'a> {
    pub(crate) path: &'a Path,
    pub(crate) parent: usize,
    pub(crate) mode: Mode,
    pub(crate) outputs: Option<CountedDirectoryOutputs>,
}
pub(crate) fn open_relative(
    handles: &mut Vec<Handle>,
    sid: &[u8],
    budget: files::Budget,
    directory: RelativeDirectory<'_>,
    context: &JobContext,
    phase: &mut NativePhase<'_>,
) -> Result<OpenedDirectory, ClaimError> {
    let RelativeDirectory { path, parent, mode, outputs } = directory;
    if matches!(budget, files::Budget::SelectedRelease(_)) && outputs.is_none() {
        return Err(budget.rejection("directory_counted_outputs_missing"));
    }
    if let Some(outputs) = &outputs {
        phase.check_budget(&outputs.primary.budget())?;
        phase.check_budget(&outputs.verification.budget())?;
    }
    phase.context(context)?;
    phase.check_budget(&budget)?;
    paths::validate_local_shape(path).map_err(|operation| phase.invalid(operation))?;
    if handles
        .len()
        .checked_add(2)
        .is_none_or(|needed| needed > HANDLE_BOUND || needed > handles.capacity())
    {
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
    let (output, verification) = match outputs {
        Some(CountedDirectoryOutputs { primary, verification, close_verification }) => {
            (Some(primary), Some((verification, close_verification)))
        }
        None => (None, None),
    };
    handles.push(output.unwrap_or_else(|| Handle::new(HANDLE::default(), budget.clone())));
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
            match mode {
                Mode::CreateNew => FILE_CREATE,
                Mode::OpenOrCreate => FILE_OPEN_IF,
                Mode::Existing => FILE_OPEN,
            },
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
    let disposition = disposition(mode, status.0, io.Information);
    if raw.0.is_null() || disposition.is_none() {
        return Err(phase.invalid("generation_creation_output"));
    }
    paths::validate_object(&handles[index], true, true, phase)?;
    let identity = paths::file_identity(&handles[index], phase)?;
    security::verify(&handles[index], sid, phase)?;
    let destination = phase.name_destination()?;
    let canonical = paths::final_path(&handles[index], phase, destination)?;
    if (mode == Mode::CreateNew && canonical.path()? != path)
        || (mode != Mode::CreateNew && canonical.path()?.parent() != path.parent())
    {
        return Err(phase.invalid("capture_generation_resolution"));
    }
    if let Some((verification, close)) = verification {
        let reopened = handles.len();
        handles.push(verification);
        paths::open_cell(canonical.path()?, true, true, &mut handles[reopened], phase)?;
        if paths::file_identity(&handles[reopened], phase)? != identity {
            return Err(phase.invalid("changed_canonical_chain"));
        }
        close(&mut handles[reopened], phase)?;
        if handles[reopened].is_armed() {
            return Err(phase.invalid("canonical_verification_still_owned"));
        }
        drop(handles.pop());
    } else {
        paths::verify_pinned(canonical.path()?, identity, handles, true, budget, phase)?;
    }
    phase.context(context)?;
    Ok(OpenedDirectory {
        index,
        identity,
        disposition: disposition.ok_or_else(|| phase.invalid("generation_creation_output"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_disposition_never_overwrites_or_admits_unknown_native_outputs() {
        assert_eq!(disposition(Mode::CreateNew, 0, 2), Some(DirectoryDisposition::Created));
        assert!(disposition(Mode::CreateNew, 0, 1).is_none());
        assert!(disposition(Mode::CreateNew, 0x40000000, 1).is_none());
        for information in [1, 2] {
            assert!(disposition(Mode::OpenOrCreate, 0, information).is_some());
        }
        assert_eq!(
            disposition(Mode::OpenOrCreate, 0x40000000, 1),
            Some(DirectoryDisposition::Existing)
        );
        assert!(disposition(Mode::OpenOrCreate, 0x40000000, 2).is_none());
        for mode in [Mode::CreateNew, Mode::OpenOrCreate] {
            for information in [0, 3, 4, 5, usize::MAX] {
                assert!(disposition(mode, 0, information).is_none());
            }
            for status in [-1, 0x40000001, 0x103] {
                assert!(disposition(mode, status, 1).is_none());
            }
        }
    }
}
