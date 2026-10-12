// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Activate bounded bytes through one retained staging inode; codecs stay with owners.
use super::{exclusion::HeldFile, paths, security};
use crate::{
    ClaimError,
    native_claims::{files::Handle, lock},
};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};
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

pub(crate) fn read<const N: usize>(file: &HeldFile<N>) -> Result<Option<[u8; N]>, ClaimError> {
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
        Err(cause) if cause.code() == ERROR_FILE_NOT_FOUND.to_hresult() => return Ok(None),
        Err(cause) => return Err(ClaimError::new("read_claim_record", cause)),
    };
    let index = handles.len();
    handles.push(Handle::new(raw, file.handle.budget()));
    let handle = &mut handles[index];
    let result = (|| {
        paths::validate_regular(handle, Some(N as i64))?;
        security::verify(handle, &file.account_sid)?;
        let mut input = handle.take_file();
        let mut bytes = [0; N];
        let result = input.read_exact(&mut bytes);
        handle.put_file(input);
        result.map_err(|e| ClaimError::new("read_claim_record", e))?;
        Ok(Some(bytes))
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
    validate_predecessor: impl FnOnce(Option<&[u8; N]>) -> Result<(), ClaimError>,
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
    let old = read(file)?;
    validate_predecessor(old.as_ref())?;
    if old.is_none() && !file.created {
        return Err(ClaimError::invalid("missing_claim_record"));
    }
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
    let mut output = stage.handle.take_file();
    let result = output
        .seek(SeekFrom::Start(0))
        .and_then(|_| output.set_len(N as u64))
        .and_then(|()| output.write_all(encoded))
        .and_then(|()| output.sync_all());
    stage.handle.put_file(output);
    result.map_err(|e| ClaimError::new("flush_claim_record", e))?;
    let target = paths::wide(&file.record_path)?;
    activate(&stage.handle, &target)?;
    stage.activated = Some(*encoded);
    finish_activation(&mut stage.activated, encoded, || flush_close(&mut stage.handle))?;
    *retained = None;
    Ok(())
}

fn flush_close(handle: &mut Handle) -> Result<(), ClaimError> {
    let output = handle.take_file();
    let result = output.sync_all().map_err(|e| ClaimError::new("flush_activated_claim", e));
    handle.put_file(output);
    result?;
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
fn activate(handle: &Handle, target: &[u16]) -> Result<(), ClaimError> {
    let units = target.len().checked_sub(1).ok_or_else(|| ClaimError::invalid("record_target"))?;
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = offset + target.len() * 2;
    let mut buffer = vec![0_usize; bytes.div_ceil(size_of::<usize>())];
    // SAFETY: Aligned owned buffer contains FILE_RENAME_INFO and the complete,
    // already bounded UTF16 destination. The stage's actual handle stays retained.
    unsafe {
        let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*information).Anonymous.ReplaceIfExists = true;
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
