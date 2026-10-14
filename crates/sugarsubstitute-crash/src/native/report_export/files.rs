// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open each selected ancestor without traversal and retain deny-delete file pins.
use super::*;
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND},
        Storage::FileSystem::*,
    },
    core::PCWSTR,
};
/// Reject reparse ancestry and pin only the actual selected regular file.
pub(super) fn open(fields: &mut Fields, intent: &ReportExportIntent) -> Result<(), E> {
    let saved = fields.target_path[7];
    fields.target_path[7] = 0;
    let result = produce(
        &fields.original,
        &mut fields.parents[0],
        &fields.target_path,
        FILE_READ_ATTRIBUTES.0,
        FILE_SHARE_READ,
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    );
    fields.target_path[7] = saved;
    result?;
    inspect(&fields.original, &fields.parents[0], true)?;
    fields.original.check_effect().map_err(E::Control)?;
    let mut root = [0; 1200];
    // SAFETY: Actual retained root handle and bounded owned UTF16 output stay live.
    // Volume-GUID spelling removes future DOS drive-map resolution from descendants.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            fields.parents[0].raw()?,
            &mut root,
            GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_GUID.0),
        )
    } as usize;
    if length == 0 {
        return Err(E::Native(windows::core::Error::from_thread()));
    }
    if length >= root.len() {
        return Err(E::Rejected(C::Destination));
    }
    fields.original.check_effect().map_err(E::Control)?;
    super::names::rebase(fields, intent, &root[..length])?;
    for index in 1..intent.directories {
        let end = fields.root_length + intent.ends[index] - 3;
        let saved = fields.target_path[end];
        fields.target_path[end] = 0;
        let result = produce(
            &fields.original,
            &mut fields.parents[index],
            &fields.target_path,
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        );
        fields.target_path[end] = saved;
        result?;
        fields.parent_count = index + 1;
        inspect(&fields.original, &fields.parents[index], true)?;
    }
    let existing = produce(
        &fields.original,
        &mut fields.target,
        &fields.target_path,
        FILE_READ_ATTRIBUTES.0 | DELETE.0,
        FILE_SHARE_READ,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
    );
    match existing {
        Ok(()) => {
            fields.target_before = Some(inspect(&fields.original, &fields.target, false)?);
            if !intent.overwrite {
                return Err(E::Rejected(C::Consent));
            }
        }
        Err(E::Native(cause))
            if cause.code() == ERROR_FILE_NOT_FOUND.to_hresult()
                || cause.code() == ERROR_PATH_NOT_FOUND.to_hresult() => {}
        Err(cause) => return Err(cause),
    }
    produce(
        &fields.original,
        &mut fields.stage,
        &fields.stage_path,
        FILE_GENERIC_WRITE.0 | FILE_READ_ATTRIBUTES.0 | DELETE.0,
        FILE_SHARE_READ,
        CREATE_NEW,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    fields.stage_before = Some(inspect(&fields.original, &fields.stage, false)?);
    Ok(())
}
/// Reserve before CreateFile and adopt accepted output before post-control checks.
fn produce(
    original: &ReportExportOrigin,
    slot: &mut Slot,
    path: &[u16],
    access: u32,
    sharing: FILE_SHARE_MODE,
    creation: FILE_CREATION_DISPOSITION,
    flags: FILE_FLAGS_AND_ATTRIBUTES,
) -> Result<(), E> {
    original.check_effect().map_err(E::Control)?;
    let output = original.reserve_output().map_err(E::Control)?;
    // SAFETY: Complete bounded NUL-terminated path remains borrowed across this synchronous
    // call; output accounting exists before the OS can return a physical file handle.
    let result =
        unsafe { CreateFileW(PCWSTR(path.as_ptr()), access, sharing, None, creation, flags, None) };
    match result {
        Ok(handle) => {
            slot.handle = Some(handle);
            slot.output = Some(output);
        }
        Err(cause) => {
            drop(output);
            return Err(E::Native(cause));
        }
    }
    original.check_effect().map_err(E::Control)
}
/// Validate same-handle object kind, reparse disposition and unique regular-file link.
fn inspect(original: &ReportExportOrigin, slot: &Slot, directory: bool) -> Result<FileFacts, E> {
    original.check_effect().map_err(E::Control)?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: Retained exact handle and initialized owned output stay live for the call.
    unsafe { GetFileInformationByHandle(slot.raw()?, &mut information) }.map_err(E::Native)?;
    let attributes = FILE_FLAGS_AND_ATTRIBUTES(information.dwFileAttributes);
    if attributes.contains(FILE_ATTRIBUTE_REPARSE_POINT)
        || attributes.contains(FILE_ATTRIBUTE_DIRECTORY) != directory
        || (!directory && information.nNumberOfLinks != 1)
    {
        return Err(E::Rejected(C::Destination));
    }
    original.check_effect().map_err(E::Control)?;
    Ok(FileFacts::from(information))
}
/// Require one complete synchronous write within the fixed64KiB stream bound.
pub(super) fn write(fields: &mut Fields, bytes: &[u8]) -> Result<(), E> {
    if bytes.is_empty() || bytes.len() > 65_536 || fields.stage_activated || fields.stage_deleted {
        return Err(E::Rejected(C::Bounds));
    }
    fields.original.check_effect().map_err(E::Control)?;
    let mut written = 0;
    // SAFETY: Immutable bounded bytes and owned write-count destination remain live;
    // exact synchronous non-overlapped stage handle is retained by this owner.
    unsafe { WriteFile(fields.stage.raw()?, Some(bytes), Some(&mut written), None) }
        .map_err(E::Native)?;
    fields.written_bytes =
        fields.written_bytes.checked_add(u64::from(written)).ok_or(E::Rejected(C::Bounds))?;
    if written as usize != bytes.len() {
        return Err(E::Rejected(C::Bounds));
    }
    fields.original.check_effect().map_err(E::Control)
}
/// Observe actual flush between original main control checks.
pub(super) fn flush(fields: &mut Fields) -> Result<(), E> {
    fields.original.check_effect().map_err(E::Control)?;
    // SAFETY: The actual writable stage stays pinned through synchronous flush.
    unsafe { FlushFileBuffers(fields.stage.raw()?) }.map_err(E::Native)?;
    fields.original.check_effect().map_err(E::Control)
}

/// Same-handle observations retained from initial open until checked physical closure.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct FileFacts {
    attributes: u32,
    links: u32,
    volume: u32,
    index: u64,
    bytes: u64,
    created: u64,
    written: u64,
}
impl From<BY_HANDLE_FILE_INFORMATION> for FileFacts {
    fn from(info: BY_HANDLE_FILE_INFORMATION) -> Self {
        Self {
            attributes: info.dwFileAttributes,
            links: info.nNumberOfLinks,
            volume: info.dwVolumeSerialNumber,
            index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            bytes: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
            created: (u64::from(info.ftCreationTime.dwHighDateTime) << 32)
                | u64::from(info.ftCreationTime.dwLowDateTime),
            written: (u64::from(info.ftLastWriteTime.dwHighDateTime) << 32)
                | u64::from(info.ftLastWriteTime.dwLowDateTime),
        }
    }
}
/// Revalidate actual predecessor facts and exact completed same-stage size before rename.
pub(super) fn validate_activation(fields: &Fields) -> Result<(), E> {
    let stage = inspect(&fields.original, &fields.stage, false)?;
    let before = fields.stage_before.ok_or(E::Rejected(C::Identity))?;
    if (stage.volume, stage.index, stage.created) != (before.volume, before.index, before.created)
        || stage.bytes != fields.written_bytes
    {
        return Err(E::Rejected(C::Identity));
    }
    if fields.target.handle.is_some() {
        let actual = inspect(&fields.original, &fields.target, false)?;
        if Some(actual) != fields.target_before {
            return Err(E::Rejected(C::Identity));
        }
    }
    Ok(())
}
