// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve pinned local filesystem objects instead of trusting lexical names.
use super::*;
use sha2::{Digest, Sha256};
use std::os::windows::ffi::OsStrExt;
use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
#[cfg(test)]
mod tests;
pub(crate) fn wide(path: &Path) -> Result<Vec<u16>, ClaimError> {
    use std::path::{Component, Prefix};
    if !path.is_absolute()
        || !matches!(path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return Err(ClaimError::invalid("non_local_path"));
    }
    if path.as_os_str().len() > 32766 * 3
        || path.as_os_str().is_empty()
        || path.as_os_str().encode_wide().count() >= 32767
    {
        return Err(ClaimError::invalid("path_shape"));
    }
    let mut units: Vec<u16> = path.as_os_str().encode_wide().collect();
    if units.contains(&0) {
        return Err(ClaimError::invalid("path_shape"));
    }
    units.push(0);
    Ok(units)
}
pub(crate) fn open_retained(
    path: &Path,
    directory: bool,
    no_reparse: bool,
    budget: files::Budget,
    handles: &mut Vec<Handle>,
) -> Result<usize, ClaimError> {
    let units = wide(path)?;
    let flags = FILE_FLAG_BACKUP_SEMANTICS
        | if no_reparse { FILE_FLAG_OPEN_REPARSE_POINT } else { FILE_FLAGS_AND_ATTRIBUTES(0) };
    let access = FILE_READ_ATTRIBUTES.0
        | READ_CONTROL.0
        | if directory { FILE_LIST_DIRECTORY.0 } else { FILE_GENERIC_READ.0 };
    // SAFETY: The owned NUL path lives through the call; no inheritable security attributes are supplied.
    let raw = unsafe {
        CreateFileW(
            PCWSTR(units.as_ptr()),
            access,
            FILE_SHARE_READ | if directory { FILE_SHARE_WRITE } else { FILE_SHARE_MODE(0) },
            None,
            OPEN_EXISTING,
            flags,
            None,
        )
    }
    .map_err(|e| ClaimError::new("resolve_path", e))?;
    let index = handles.len();
    handles.push(Handle::new(raw, budget));
    validate_object(&handles[index], directory, no_reparse)?;
    Ok(index)
}
fn info<T: Default>(handle: &Handle, class: FILE_INFO_BY_HANDLE_CLASS) -> Result<T, ClaimError> {
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
    .map_err(|e| ClaimError::new("path_identity", e))?;
    Ok(value)
}
fn validate_object(handle: &Handle, directory: bool, no_reparse: bool) -> Result<(), ClaimError> {
    // SAFETY: The retained owner keeps this handle valid throughout metadata queries.
    if unsafe { GetFileType(handle.raw()) } != FILE_TYPE_DISK {
        return Err(ClaimError::invalid("non_disk_path"));
    }
    let standard: FILE_STANDARD_INFO = info(handle, FileStandardInfo)?;
    let attributes: FILE_ATTRIBUTE_TAG_INFO = info(handle, FileAttributeTagInfo)?;
    if standard.Directory != directory
        || standard.DeletePending
        || (no_reparse && attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0)
    {
        return Err(ClaimError::invalid("path_object_type"));
    }
    let mut flags = 0;
    // SAFETY: The retained file handle is valid and the volume-flags output is initialized.
    unsafe {
        GetVolumeInformationByHandleW(handle.raw(), None, None, None, Some(&mut flags), None)
    }
    .map_err(|e| ClaimError::new("path_volume", e))?;
    // FILE_SUPPORTS_OPEN_BY_FILE_ID, defined by the pinned Windows SDK.
    if flags & 0x01000000 != 0x01000000 {
        return Err(ClaimError::invalid("unverified_path_volume"));
    }
    let final_name = final_path(handle)?;
    let text = final_name.to_str().ok_or_else(|| ClaimError::invalid("path_unicode"))?;
    if !text.starts_with("\\\\?\\")
        || text.starts_with("\\\\?\\UNC\\")
        || text.as_bytes().get(5) != Some(&b':')
    {
        return Err(ClaimError::invalid("non_local_path"));
    }
    Ok(())
}
pub(crate) fn validate_regular(handle: &Handle, length: Option<i64>) -> Result<(), ClaimError> {
    validate_object(handle, false, true)?;
    let standard: FILE_STANDARD_INFO = info(handle, FileStandardInfo)?;
    if standard.NumberOfLinks != 1 || length.is_some_and(|value| value != standard.EndOfFile) {
        return Err(ClaimError::invalid("claim_file_shape"));
    }
    Ok(())
}
pub(crate) fn final_path(handle: &Handle) -> Result<PathBuf, ClaimError> {
    let mut units = vec![0; 32767];
    // SAFETY: The handle remains retained and the complete writable slice is advertised to Windows.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            handle.raw(),
            &mut units,
            GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
        )
    };
    if length == 0 {
        return Err(ClaimError::new("canonical_path", std::io::Error::last_os_error()));
    }
    if length as usize >= units.len() {
        return Err(ClaimError::invalid("canonical_path_bound"));
    }
    units.truncate(length as usize);
    String::from_utf16(&units)
        .map(PathBuf::from)
        .map_err(|e| ClaimError::new("canonical_path_unicode", e))
}
pub(crate) fn file_identity(handle: &Handle) -> Result<[u8; 24], ClaimError> {
    let info: FILE_ID_INFO = info(handle, FileIdInfo)?;
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
) -> Result<(), ClaimError> {
    let reopened = open_retained(path, directory, true, budget, handles)?;
    let same = file_identity(&handles[reopened])? == identity;
    if same { Ok(()) } else { Err(ClaimError::invalid("changed_canonical_chain")) }
}
pub(crate) fn hash(parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}
pub(crate) fn resolve(resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError> {
    let budget = files::Budget::Slot(resolution.slot.clone());
    let mut handles = lock(&resolution.handles);
    let root =
        open_retained(resolution.configuration.root(), true, false, budget.clone(), &mut handles)?;
    let root_path = final_path(&handles[root])?;
    let root_identity = file_identity(&handles[root])?;
    pin_ancestors(&root_path, &mut handles, files::Budget::Slot(resolution.slot.clone()))?;
    let interpreter =
        open_retained(resolution.configuration.interpreter(), false, false, budget, &mut handles)?;
    let interpreter_path = final_path(&handles[interpreter])?;
    let interpreter_identity = file_identity(&handles[interpreter])?;
    pin_ancestors(&interpreter_path, &mut handles, files::Budget::Slot(resolution.slot.clone()))?;
    verify_pinned(
        &root_path,
        root_identity,
        &mut handles,
        true,
        files::Budget::Slot(resolution.slot.clone()),
    )?;
    verify_pinned(
        &interpreter_path,
        interpreter_identity,
        &mut handles,
        false,
        files::Budget::Slot(resolution.slot.clone()),
    )?;
    let canonical = LaunchConfiguration::new(
        root_path,
        interpreter_path,
        resolution.configuration.arguments().to_vec(),
        resolution.configuration.port().get(),
    )
    .map_err(|e| ClaimError::new("canonical_configuration", e))?;
    resolution.digest = backend_hash(sid, &root_identity, &interpreter_identity, &canonical);
    drop(handles);
    resolution.configuration = canonical;
    resolution.account_sid = sid.to_vec();
    resolution.ready = true;
    Ok(())
}
fn backend_hash(
    sid: &[u8],
    root_identity: &[u8; 24],
    interpreter_identity: &[u8; 24],
    canonical: &LaunchConfiguration,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in
        [b"SugarSubstitute.backend-claim.v1".as_slice(), sid, root_identity, interpreter_identity]
    {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.update((canonical.arguments().len() as u64).to_le_bytes());
    for argument in canonical.arguments() {
        hash.update((argument.len() as u64).to_le_bytes());
        hash.update(argument.as_bytes());
    }
    hash.update(canonical.port().get().to_le_bytes());
    hash.finalize().into()
}
pub(crate) fn pin_ancestors(
    path: &Path,
    handles: &mut Vec<Handle>,
    budget: files::Budget,
) -> Result<(), ClaimError> {
    let parent = path.parent().ok_or_else(|| ClaimError::invalid("path_parent"))?;
    if parent.components().count() > 128 {
        return Err(ClaimError::invalid("path_component_bound"));
    }
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        if current.has_root() {
            open_retained(&current, true, true, budget.clone(), handles)?;
        }
    }
    Ok(())
}
pub(crate) fn create_directory(
    path: &Path,
    sid: &[u8],
    budget: files::Budget,
    handles: &mut Vec<Handle>,
) -> Result<(), ClaimError> {
    let units = wide(path)?;
    let descriptor = security::Descriptor::new(sid)?;
    let attributes = descriptor.attributes();
    // SAFETY: The owned NUL path and security descriptor remain valid for the entire call.
    let result = match unsafe { CreateDirectoryW(PCWSTR(units.as_ptr()), Some(&attributes)) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS.to_hresult() => {
            Ok(())
        }
        Err(e) => Err(ClaimError::new("claim_directory", e)),
    };
    result?;
    let index = open_retained(path, true, true, budget, handles)?;
    security::verify(&handles[index], sid)
}
