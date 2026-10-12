// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Activate bounded bytes through one retained staging inode; codecs stay with owners.
mod streaming;
use super::{exclusion::HeldFile, paths, security};
use crate::{
    ClaimError,
    native_claims::{files::Handle, lock},
};
use std::path::PathBuf;
pub(crate) use streaming::StreamingPort;
pub(crate) use streaming::layout::binding as diagnostic_object_binding;
pub(crate) use streaming::layout::subject as diagnostic_subject_digest;
use windows::{
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, Security::Cryptography::*, Storage::FileSystem::*},
    core::PCWSTR,
};

pub(crate) struct Stage<const N: usize> {
    pub(crate) handle: Handle,
    path: PathBuf,
    identity: Option<[u8; 24]>,
    activated: Option<[u8; N]>,
}

pub(crate) enum RecordRead<const N: usize> {
    Missing,
    Empty,
    Present([u8; N]),
}
pub(crate) fn read_shape<const N: usize>(file: &HeldFile<N>) -> Result<RecordRead<N>, ClaimError> {
    read_shape_bound(file, None)
}
fn read_shape_bound<const N: usize>(
    file: &HeldFile<N>,
    expected_identity: Option<[u8; 24]>,
) -> Result<RecordRead<N>, ClaimError> {
    let path = paths::wide(&file.record_path)?;
    let mut handles = lock(&file.temporary_handles);
    if handles.len() >= 16 || !(16..=1024).contains(&N) {
        return Err(ClaimError::invalid("temporary_handle_bound"));
    }
    handles.try_reserve_exact(1).map_err(|e| ClaimError::new("record_handle_reservation", e))?;
    // SAFETY: The bounded NUL path lives through OPEN_EXISTING, which cannot mutate data.
    let raw = match unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    } {
        Ok(raw) => raw,
        Err(cause) if cause.code() == ERROR_FILE_NOT_FOUND.to_hresult() => {
            return Ok(RecordRead::Missing);
        }
        Err(cause) => return Err(ClaimError::new("read_claim_record", cause)),
    };
    let index = handles.len();
    handles.push(Handle::new(raw, file.handle.budget()));
    let handle = &mut handles[index];
    let result = (|| {
        paths::validate_regular(handle, None)?;
        security::verify(handle, &file.account_sid)?;
        if let Some(expected) = expected_identity
            && paths::file_identity(handle)? != expected
        {
            return Err(ClaimError::invalid("changed_retirement_record_identity"));
        }
        let mut length = 0_i64;
        // SAFETY: The retained source and exact initialized length output stay live.
        unsafe { GetFileSizeEx(handle.raw(), &mut length) }
            .map_err(|cause| ClaimError::new("claim_record_shape", cause))?;
        if length == 0 {
            return Ok(RecordRead::Empty);
        }
        if length != N as i64 {
            return Err(ClaimError::invalid("claim_file_shape"));
        }
        let mut bytes = [0; N];
        let mut count = 0;
        // SAFETY: Exact initialized fixed output and synchronous source remain owned.
        unsafe { ReadFile(handle.raw(), Some(&mut bytes), Some(&mut count), None) }
            .map_err(|cause| ClaimError::new("read_claim_record", cause))?;
        if count as usize != N {
            return Err(ClaimError::invalid("short_claim_record"));
        }
        Ok(RecordRead::Present(bytes))
    })();
    let cleanup = handle.close();
    if cleanup.is_ok() {
        handles.remove(index);
    }
    ClaimError::finish(result, cleanup)
}

pub(crate) fn publish<const N: usize>(
    file: &HeldFile<N>,
    encoded: &[u8; N],
    validate_predecessor: impl FnOnce(&RecordRead<N>) -> Result<(), ClaimError>,
) -> Result<(), ClaimError> {
    if !file.is_locked() || !(16..=1024).contains(&N) {
        return Err(ClaimError::invalid("publication_without_exclusion"));
    }
    let mut retained = lock(&file.stage);
    if let Some(stage) = retained.as_mut()
        && stage.activated.is_some()
    {
        let matched =
            finish_activation(&mut stage.activated, encoded, || flush_close(&mut stage.handle))?;
        *retained = None;
        if matched {
            return Ok(());
        }
    }
    let old = read_shape(file)?;
    match &old {
        RecordRead::Empty => return Err(ClaimError::invalid("empty_claim_record")),
        RecordRead::Missing if !file.created => {
            return Err(ClaimError::invalid("missing_claim_record"));
        }
        _ => (),
    }
    validate_predecessor(&old)?;
    if retained.is_none() {
        let path = file.record_path.with_extension(format!("{}.stage", hex(&random()?)));
        let wide = paths::wide(&path)?;
        let descriptor = security::Descriptor::new(&file.account_sid)?;
        let attributes = descriptor.attributes();
        // SAFETY: Fresh bounded name and noninheritable protected descriptor live
        // through CREATE_NEW. The returned handle is retained before all validation.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | DELETE.0,
                FILE_SHARE_READ,
                Some(&attributes),
                CREATE_NEW,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|e| ClaimError::new("stage_claim_record", e))?;
        *retained = Some(Stage {
            handle: Handle::new(raw, file.handle.budget()),
            path,
            identity: None,
            activated: None,
        });
    }
    let stage = retained.as_mut().ok_or_else(|| ClaimError::invalid("missing_stage"))?;
    let wide = paths::wide(&stage.path)?;
    if stage.handle.raw().0.is_null() {
        // SAFETY: Only this retained stage name is reopened; actual identity is checked before writes.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | DELETE.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|e| ClaimError::new("reopen_claim_stage", e))?;
        stage.handle = Handle::new(raw, file.handle.budget());
    }
    let identity = paths::file_identity(&stage.handle)?;
    if stage.identity.is_some_and(|previous| previous != identity) {
        return Err(ClaimError::invalid("changed_claim_stage"));
    }
    stage.identity = Some(identity);
    paths::validate_regular(&stage.handle, None)?;
    security::verify(&stage.handle, &file.account_sid)?;
    let mut written = 0;
    // SAFETY: The exact retained writable stage remains armed throughout synchronous
    // bounded seek/truncate/write/flush; no temporary File can drop its native custody.
    unsafe {
        SetFilePointerEx(stage.handle.raw(), 0, None, FILE_BEGIN)
            .map_err(|cause| ClaimError::new("seek_claim_record", cause))?;
        SetEndOfFile(stage.handle.raw())
            .map_err(|cause| ClaimError::new("truncate_claim_record", cause))?;
        WriteFile(stage.handle.raw(), Some(encoded), Some(&mut written), None)
            .map_err(|cause| ClaimError::new("write_claim_record", cause))?;
        if written as usize != N {
            return Err(ClaimError::invalid("short_claim_write"));
        }
        FlushFileBuffers(stage.handle.raw())
            .map_err(|cause| ClaimError::new("flush_claim_record", cause))?;
    }
    let target = paths::wide(&file.record_path)?;
    activate(&stage.handle, &target, true)?;
    stage.path = file.record_path.clone();
    stage.activated = Some(*encoded);
    finish_activation(&mut stage.activated, encoded, || flush_close(&mut stage.handle))?;
    *retained = None;
    Ok(())
}

fn flush_close(handle: &mut Handle) -> Result<(), ClaimError> {
    // SAFETY: The actual retained writable owner remains armed through flush.
    unsafe { FlushFileBuffers(handle.raw()) }
        .map_err(|cause| ClaimError::new("flush_activated_claim", cause))?;
    handle.close()
}
pub(crate) fn finish_activation<const N: usize>(
    state: &mut Option<[u8; N]>,
    desired: &[u8; N],
    cleanup: impl FnOnce() -> Result<(), ClaimError>,
) -> Result<bool, ClaimError> {
    let Some(actual) = state.as_ref() else {
        return Ok(false);
    };
    let matched = actual == desired;
    cleanup()?;
    *state = None;
    Ok(matched)
}
fn activate(handle: &Handle, target: &[u16], replace: bool) -> Result<(), ClaimError> {
    let units = target.len().checked_sub(1).ok_or_else(|| ClaimError::invalid("record_target"))?;
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = offset + target.len() * 2;
    let mut buffer = vec![0_usize; bytes.div_ceil(size_of::<usize>())];
    // SAFETY: Aligned owned buffer contains FILE_RENAME_INFO and the complete,
    // already bounded UTF16 destination. The stage's actual handle stays retained.
    unsafe {
        let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*information).Anonymous.ReplaceIfExists = replace;
        (*information).FileNameLength = (units * 2) as u32;
        std::ptr::copy_nonoverlapping(
            target.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast(),
            target.len(),
        );
        SetFileInformationByHandle(
            handle.raw(),
            FileRenameInfo,
            buffer.as_ptr().cast(),
            bytes as u32,
        )
    }
    .map_err(|e| ClaimError::new("activate_claim_record", e))
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|byte| {
            [DIGITS[(byte >> 4) as usize] as char, DIGITS[(byte & 15) as usize] as char]
        })
        .collect()
}
pub(crate) fn random() -> Result<[u8; 16], ClaimError> {
    let mut bytes = [0; 16];
    // SAFETY: The exact initialized writable slice belongs to this bounded nonce operation.
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(|e| ClaimError::new("claim_randomness", e))?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(ClaimError::invalid("claim_randomness"));
    }
    Ok(bytes)
}
