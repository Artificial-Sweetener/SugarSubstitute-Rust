// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native file production and strict validation before publishing retained output indices.
use super::{NativePhase, canonical_path};
use crate::{
    ClaimError,
    claim_error::BoundedNativeCause,
    native_claims::files::{self, Handle},
};
use std::path::Path;
use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
#[cfg(test)]
mod tests;

/// Adopt a generic filesystem output and validate its selected object before returning its index.
pub(crate) fn open_retained(
    path: &Path,
    directory: bool,
    no_reparse: bool,
    budget: files::Budget,
    handles: &mut Vec<Handle>,
    phase: &mut NativePhase<'_>,
) -> Result<usize, ClaimError> {
    let index = reserve_output(budget, handles, phase)?;
    open_cell(path, directory, no_reparse, &mut handles[index], phase)?;
    Ok(index)
}
/// Adopt and strictly validate one regular file before returning its retained output index.
/// Initial path following matches generic opens; strict object/reparse/volume/path and
/// single-link checks run once on the same adopted handle. Rejection retains that output
/// for explicit caller-owned closure. This does not replace canonical reopen verification.
pub(crate) fn open_regular_retained(
    path: &Path,
    budget: files::Budget,
    handles: &mut Vec<Handle>,
    phase: &mut NativePhase<'_>,
) -> Result<usize, ClaimError> {
    let index = reserve_output(budget, handles, phase)?;
    open_handle(path, false, false, &mut handles[index], phase)?;
    validate_regular(&handles[index], None, phase)?;
    Ok(index)
}

/// Retain an empty output cell before any native open can return an owned handle.
fn reserve_output(
    budget: files::Budget,
    handles: &mut Vec<Handle>,
    phase: &mut NativePhase<'_>,
) -> Result<usize, ClaimError> {
    phase.check_budget(&budget)?;
    if handles.len() == handles.capacity() {
        if !budget.is_ordinary() {
            return Err(budget.rejection("retained_handle_capacity"));
        }
        handles.try_reserve_exact(1).map_err(|cause| {
            phase.error("retained_handle_reservation", BoundedNativeCause::Reserve(cause))
        })?;
    }
    let index = handles.len();
    handles.push(Handle::new(windows::Win32::Foundation::HANDLE::default(), budget));
    Ok(index)
}

/// Open into the caller's already reserved physical output, preserving adoption order.
/// The same producer serves ordinary, Diagnostic and actual selected capture owners.
pub(crate) fn open_cell(
    path: &Path,
    directory: bool,
    no_reparse: bool,
    handle: &mut Handle,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    open_handle(path, directory, no_reparse, handle, phase)?;
    validate_object(handle, directory, no_reparse, phase)
}

/// Produce only an adopted native output; complete validation stays with this module's callers.
fn open_handle(
    path: &Path,
    directory: bool,
    no_reparse: bool,
    handle: &mut Handle,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    phase.check_budget(&handle.budget())?;
    let units = phase.wide(path)?;
    units.check()?;
    let flags = FILE_FLAG_BACKUP_SEMANTICS
        | if no_reparse { FILE_FLAG_OPEN_REPARSE_POINT } else { FILE_FLAGS_AND_ATTRIBUTES(0) };
    let access = FILE_READ_ATTRIBUTES.0
        | READ_CONTROL.0
        | if directory { FILE_LIST_DIRECTORY.0 } else { FILE_GENERIC_READ.0 };
    // SAFETY: The owned NUL path lives through the call; no inheritable security attributes are supplied.
    let opened = unsafe {
        CreateFileW(
            PCWSTR(units.units().as_ptr()),
            access,
            FILE_SHARE_READ
                | if directory && handle.budget().is_ordinary() {
                    FILE_SHARE_WRITE
                } else {
                    FILE_SHARE_MODE(0)
                },
            None,
            OPEN_EXISTING,
            flags,
            None,
        )
    };
    match opened {
        Ok(raw) => handle.adopt_output(raw),
        Err(cause) => {
            handle.adopt_output(windows::Win32::Foundation::HANDLE::default());
            drop(units);
            return Err(phase.error("resolve_path", BoundedNativeCause::Windows(cause)));
        }
    }
    drop(units);
    Ok(())
}
/// Query one native information structure under the retained handle's current phase and budget.
pub(super) fn info<T: Default>(
    handle: &Handle,
    class: FILE_INFO_BY_HANDLE_CLASS,
    phase: &mut NativePhase<'_>,
) -> Result<T, ClaimError> {
    phase.check_budget(&handle.budget())?;
    phase.check()?;
    let mut value = T::default();
    // SAFETY: T is an initialized native information structure of exactly this advertised size.
    unsafe {
        GetFileInformationByHandleEx(
            handle.raw(),
            class,
            (&mut value as *mut T).cast(),
            size_of::<T>() as u32,
        )
    }
    .map_err(|cause| phase.error("path_identity", BoundedNativeCause::Windows(cause)))?;
    Ok(value)
}
/// Require the selected disk object, stable shape, supported volume and canonical local path.
pub(crate) fn validate_object(
    handle: &Handle,
    directory: bool,
    no_reparse: bool,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    phase.check_budget(&handle.budget())?;
    phase.check()?;
    // SAFETY: The retained owner keeps this handle valid throughout metadata queries.
    if unsafe { GetFileType(handle.raw()) } != FILE_TYPE_DISK {
        return Err(phase.invalid("non_disk_path"));
    }
    let standard: FILE_STANDARD_INFO = info(handle, FileStandardInfo, phase)?;
    let attributes: FILE_ATTRIBUTE_TAG_INFO = info(handle, FileAttributeTagInfo, phase)?;
    if standard.Directory != directory
        || standard.DeletePending
        || (no_reparse && attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0)
    {
        return Err(phase.invalid("path_object_type"));
    }
    let mut flags = 0;
    phase.check()?;
    // SAFETY: The retained file handle is valid and the volume-flags output is initialized.
    unsafe {
        GetVolumeInformationByHandleW(handle.raw(), None, None, None, Some(&mut flags), None)
    }
    .map_err(|cause| phase.error("path_volume", BoundedNativeCause::Windows(cause)))?;
    // FILE_SUPPORTS_OPEN_BY_FILE_ID, defined by the pinned Windows SDK.
    if flags & 0x01000000 != 0x01000000 {
        return Err(phase.invalid("unverified_path_volume"));
    }
    let canonical = canonical_path(handle, phase)?;
    drop(canonical);
    phase.check()
}
/// Require a strict regular object with one hard link and any supplied exact byte length.
pub(crate) fn validate_regular(
    handle: &Handle,
    length: Option<i64>,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    validate_object(handle, false, true, phase)?;
    let standard: FILE_STANDARD_INFO = info(handle, FileStandardInfo, phase)?;
    if standard.NumberOfLinks != 1 || length.is_some_and(|value| value != standard.EndOfFile) {
        return Err(phase.invalid("claim_file_shape"));
    }
    Ok(())
}
