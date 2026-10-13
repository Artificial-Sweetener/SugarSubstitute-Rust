// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain and validate canonical local directory chains for protected storage.
use super::{
    native_name::{NativeName, NativeNameDestination},
    native_phase::NativePhase,
    security,
};
use crate::claim_error::BoundedNativeCause;
use crate::{
    ClaimError,
    native_claims::files::{self, Handle},
};
use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use windows::{
    Win32::Storage::FileSystem::*,
    core::{PCWSTR, PWSTR},
};
#[cfg(test)]
mod tests;
/// Validate the shared bounded local path shape without allocating native units.
pub(crate) fn validate_local_shape(path: &Path) -> Result<(), &'static str> {
    use std::path::{Component, Prefix};
    if !path.is_absolute()
        || !matches!(path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return Err("non_local_path");
    }
    if path.as_os_str().len() > 32766 * 3
        || path.as_os_str().is_empty()
        || path.as_os_str().encode_wide().count() >= 32767
    {
        return Err("path_shape");
    }
    if path.as_os_str().encode_wide().any(|unit| unit == 0) {
        return Err("path_shape");
    }
    Ok(())
}
/// Allocate one native NUL path only after the shared shape validation succeeds.
pub(crate) fn wide(path: &Path) -> Result<Vec<u16>, ClaimError> {
    validate_local_shape(path).map_err(ClaimError::invalid)?;
    let mut units: Vec<u16> = path.as_os_str().encode_wide().collect();
    units.push(0);
    Ok(units)
}
pub(crate) fn open_retained(
    path: &Path,
    directory: bool,
    no_reparse: bool,
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
    open_cell(path, directory, no_reparse, &mut handles[index], phase)?;
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
    validate_object(handle, directory, no_reparse, phase)?;
    Ok(())
}
fn info<T: Default>(
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
pub(crate) fn final_path(
    handle: &Handle,
    phase: &mut NativePhase<'_>,
    destination: NativeNameDestination,
) -> Result<NativeName, ClaimError> {
    phase.check_destination(&destination)?;
    let path = canonical_path(handle, phase)?;
    destination.finish(path)
}
fn canonical_path(handle: &Handle, phase: &mut NativePhase<'_>) -> Result<PathBuf, ClaimError> {
    phase.check_budget(&handle.budget())?;
    phase.path()?;
    let mut units = vec![0; 32767];
    phase.check()?;
    // SAFETY: The handle remains retained and the complete writable slice is advertised to Windows.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            handle.raw(),
            &mut units,
            GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
        )
    };
    if length == 0 {
        return Err(phase
            .error("canonical_path", BoundedNativeCause::RawIo(std::io::Error::last_os_error())));
    }
    if length as usize >= units.len() {
        return Err(phase.invalid("canonical_path_bound"));
    }
    let path = decode_native_result(&units[..length as usize])
        .map_err(|operation| phase.invalid(operation))?;
    drop(units);
    phase.check()?;
    Ok(path)
}
/// Decode bounded actual normalized Windows path units without scalar conversion.
fn decode_native_result(units: &[u16]) -> Result<PathBuf, &'static str> {
    if units.is_empty() || units.len() >= 32767 || units.contains(&0) {
        return Err("canonical_path_bound");
    }
    let path = PathBuf::from(OsString::from_wide(units));
    validate_canonical_local(&path)?;
    Ok(path)
}
/// Require the existing local verbatim drive authority shape.
fn validate_canonical_local(path: &Path) -> Result<(), &'static str> {
    use std::path::{Component, Prefix};
    validate_local_shape(path)?;
    if !matches!(path.components().next(),Some(Component::Prefix(prefix)) if matches!(prefix.kind(),Prefix::VerbatimDisk(_)))
    {
        return Err("non_local_path");
    }
    let mut units = path.as_os_str().encode_wide();
    if units.next() != Some(92)
        || units.next() != Some(92)
        || units.next() != Some(63)
        || units.next() != Some(92)
        || !units.next().is_some_and(|unit| (65..=90).contains(&unit) || (97..=122).contains(&unit))
        || units.next() != Some(58)
        || units.next() != Some(92)
    {
        return Err("non_local_path");
    }
    Ok(())
}
/// Read the actual Shell-owned NUL path while its documented allocation is live.
/// Errors preserve the original caller-owned allocation for checked allocator release.
/// # Safety
/// The pointer remains valid for each unit before its first NUL, or for all `maximum`
/// units when unterminated. The caller retains allocator ownership on every outcome.
pub(crate) unsafe fn read_native_path(text: PWSTR, maximum: usize) -> Result<PathBuf, ClaimError> {
    if text.is_null() || maximum == 0 || maximum > 32767 {
        return Err(ClaimError::invalid("native_path_bound"));
    }
    let mut units = Vec::new();
    units.try_reserve_exact(maximum).map_err(|e| ClaimError::new("native_path_reservation", e))?;
    for index in 0..maximum {
        // SAFETY: The caller's valid native allocation stays live and ends at NUL.
        let unit = unsafe { text.0.add(index).read() };
        if unit == 0 {
            let path = PathBuf::from(OsString::from_wide(&units));
            validate_local_shape(&path).map_err(ClaimError::invalid)?;
            return Ok(path);
        }
        units.push(unit);
    }
    Err(ClaimError::invalid("native_path_bound"))
}

pub(crate) fn file_identity(
    handle: &Handle,
    phase: &mut NativePhase<'_>,
) -> Result<[u8; 24], ClaimError> {
    let info: FILE_ID_INFO = info(handle, FileIdInfo, phase)?;
    let mut bytes = [0; 24];
    bytes[..8].copy_from_slice(&info.VolumeSerialNumber.to_le_bytes());
    bytes[8..].copy_from_slice(&info.FileId.Identifier);
    Ok(bytes)
}
pub(crate) fn verify_pinned(
    path: &Path,
    identity: [u8; 24],
    handles: &mut Vec<Handle>,
    directory: bool,
    budget: files::Budget,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    let reopened = open_retained(path, directory, true, budget, handles, phase)?;
    let same = file_identity(&handles[reopened], phase)? == identity;
    if same { Ok(()) } else { Err(phase.invalid("changed_canonical_chain")) }
}
pub(crate) fn pin_ancestors(
    path: &Path,
    handles: &mut Vec<Handle>,
    budget: files::Budget,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    phase.check_budget(&budget)?;
    for current in ancestor_paths(path).map_err(|operation| phase.invalid(operation))? {
        open_retained(current, true, true, budget.clone(), handles, phase)?;
    }
    Ok(())
}

/// Yield actual ancestor directories, including the drive root, in retention order.
/// A verbatim drive prefix reports a root lexically before its separator exists;
/// that incomplete prefix cannot name a directory in CreateFileW.
pub(crate) fn ancestor_paths(
    path: &Path,
) -> Result<impl Iterator<Item = &Path> + '_, &'static str> {
    let parent = path.parent().ok_or("path_parent")?;
    if parent.components().count() > 128 {
        return Err("path_component_bound");
    }
    let depths = parent.ancestors().count();
    Ok((0..depths).rev().filter_map(move |depth| {
        parent.ancestors().nth(depth).filter(|current| {
            current.has_root()
                && !matches!(
                    current.components().next_back(),
                    Some(std::path::Component::Prefix(_))
                )
        })
    }))
}
pub(crate) fn create_directory(
    path: &Path,
    sid: &[u8],
    budget: files::Budget,
    handles: &mut Vec<Handle>,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    phase.check_budget(&budget)?;
    let producing = phase.open_buffers(path, Some(sid))?;
    let attributes = producing.attributes();
    producing.check()?;
    // SAFETY: The same exclusive producing scope retains name and descriptor until
    // the actual directory result is adopted; creation never replaces an object.
    let result = unsafe {
        CreateDirectoryW(
            PCWSTR(producing.units().as_ptr()),
            attributes.as_ref().map(|value| value as *const _),
        )
    };
    drop(producing);
    match result {
        Ok(()) => {}
        Err(cause)
            if cause.code() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS.to_hresult() => {}
        Err(cause) => {
            return Err(phase.error("claim_directory", BoundedNativeCause::Windows(cause)));
        }
    }
    let index = open_retained(path, true, true, budget, handles, phase)?;
    security::verify(&handles[index], sid, phase)
}

/// Describe the fixed bounded path conversion and its actual session controls.
/// Raw UTF16 and both UTF8 replacement destinations overlap during decoding.
pub(crate) fn diagnostic_path_layout() -> Result<
    crate::native_claims::files::NativeRoleLayout,
    crate::native_claims::files::NativeLayoutFailure,
> {
    use crate::{
        diagnostic_storage::{DiagnosticNativePathPhase, DiagnosticNativeScratchLoan},
        native_claims::files::{NativeLayoutFailure, NativeRoleLayout},
    };
    let controls = size_of::<Vec<u16>>()
        .checked_add(2 * size_of::<PathBuf>())
        .and_then(|value| value.checked_add(size_of::<DiagnosticNativeScratchLoan>()))
        .and_then(|value| value.checked_add(size_of::<DiagnosticNativePathPhase<'_>>()))
        .ok_or(NativeLayoutFailure::Overflow)?;
    let peak =
        (65_534_usize + 196_596).checked_add(controls).ok_or(NativeLayoutFailure::Overflow)?;
    Ok(NativeRoleLayout { retained: 0, peak })
}
