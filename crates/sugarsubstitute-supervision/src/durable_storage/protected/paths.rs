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
use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
mod object;
use object::info;
pub(crate) use object::{
    open_cell, open_regular_retained, open_retained, validate_object, validate_regular,
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
        || crate::durable_storage::files::native_units(path).count() >= 32767
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
    let mut units: Vec<u16> = crate::durable_storage::files::native_units(path).collect();
    units.push(0);
    Ok(units)
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
