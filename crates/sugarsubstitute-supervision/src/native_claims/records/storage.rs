// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one identifiable staging object through checked flush/close/activation retries.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};
use windows::{
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, Storage::FileSystem::*},
    core::PCWSTR,
};
#[cfg(test)]
mod tests;
pub(crate) struct Stage {
    pub(crate) handle: Handle,
    path: PathBuf,
    identity: Option<[u8; 24]>,
    activated: Option<[u8; 128]>,
}
fn read(file: &HeldFile, sid: &[u8]) -> Result<Option<Record>, ClaimError> {
    let path = paths::wide(&file.record_path)?;
    let mut handles = lock(&file.temporary_handles);
    if handles.len() >= 16 {
        return Err(ClaimError::invalid("temporary_handle_bound"));
    }
    // SAFETY: The NUL path remains alive; OPEN_EXISTING cannot mutate the target.
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
        paths::validate_regular(handle, Some(128))?;
        security::verify(handle, sid)?;
        let mut input = handle.take_file();
        let mut bytes = [0; 128];
        let result = input.read_exact(&mut bytes);
        handle.put_file(input);
        result.map_err(|e| ClaimError::new("read_claim_record", e))?;
        Record::decode(&bytes).map(Some)
    })();
    let cleanup = handle.close();
    if cleanup.is_ok() {
        handles.remove(index);
    }
    ClaimError::finish(result, cleanup)
}
pub(crate) fn check_predecessor(
    file: &HeldFile,
    sid: &[u8],
    expected: &Record,
) -> Result<(), ClaimError> {
    match read(file, sid)? {
        Some(record) if record.state == RecordState::Released && same_claim(&record, expected) => {
            Ok(())
        }
        None if file.created => Ok(()),
        _ => Err(ClaimError::invalid("unresolved_claim_predecessor")),
    }
}
pub(crate) fn publish(file: &HeldFile, record: &Record) -> Result<(), ClaimError> {
    if !file.is_locked() {
        return Err(ClaimError::invalid("publication_without_exclusion"));
    }
    let sid = &file.account_sid;
    let encoded = record.encode();
    let mut retained = lock(&file.stage);
    if let Some(stage) = retained.as_mut()
        && stage.activated.is_some()
    {
        let matched =
            finish_activation(&mut stage.activated, &encoded, || flush_close(&mut stage.handle))?;
        *retained = None;
        if matched {
            return Ok(());
        }
    }
    if let Some(old) = read(file, sid)? {
        if !permits_replacement(&old, record) {
            return Err(ClaimError::invalid("changed_claim_generation"));
        }
    } else if !file.created {
        return Err(ClaimError::invalid("missing_claim_record"));
    }
    if retained.is_none() {
        let path = file.record_path.with_extension(format!("{}.stage", hex(&random()?)));
        let wide = paths::wide(&path)?;
        let descriptor = security::Descriptor::new(sid)?;
        let attributes = descriptor.attributes();
        // SAFETY: The fresh NUL path and noninheritable descriptor stay live through CREATE_NEW.
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
        .map_err(|e| ClaimError::new("stage_claim_record", e));
        let result = match raw {
            Ok(raw) => {
                *retained = Some(Stage {
                    handle: Handle::new(raw, file.handle.budget()),
                    path,
                    identity: None,
                    activated: None,
                });
                Ok(())
            }
            Err(cause) => Err(cause),
        };
        result?;
    }
    let stage = retained.as_mut().ok_or_else(|| ClaimError::invalid("missing_stage"))?;
    let wide = paths::wide(&stage.path)?;
    if stage.handle.raw().0.is_null() {
        // SAFETY: Reopen only this owner's retained staging name; its identity is verified before writes.
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
    let result = (|| {
        let identity = paths::file_identity(&stage.handle)?;
        if stage.identity.is_some_and(|previous| previous != identity) {
            return Err(ClaimError::invalid("changed_claim_stage"));
        }
        stage.identity = Some(identity);
        paths::validate_regular(&stage.handle, None)?;
        security::verify(&stage.handle, sid)?;
        let mut output = stage.handle.take_file();
        let result = output
            .seek(SeekFrom::Start(0))
            .and_then(|_| output.set_len(128))
            .and_then(|()| output.write_all(&encoded))
            .and_then(|()| output.sync_all());
        stage.handle.put_file(output);
        result.map_err(|e| ClaimError::new("flush_claim_record", e))
    })();
    result?;
    let target = paths::wide(&file.record_path)?;
    activate(&stage.handle, &target)?;
    stage.activated = Some(encoded);
    finish_activation(&mut stage.activated, &encoded, || flush_close(&mut stage.handle))?;
    *retained = None;
    Ok(())
}
fn same_claim(old: &Record, new: &Record) -> bool {
    old.digest == new.digest && (old.resource == 0) == (new.resource == 0)
}
fn permits_replacement(old: &Record, new: &Record) -> bool {
    same_claim(old, new)
        && (old.state == RecordState::Released
            || (old.owner == new.owner
                && old.resource == new.resource
                && old.generation == new.generation))
}
fn flush_close(handle: &mut Handle) -> Result<(), ClaimError> {
    let output = handle.take_file();
    let result = output.sync_all().map_err(|e| ClaimError::new("flush_activated_claim", e));
    handle.put_file(output);
    // A failed flush keeps the actual renamed handle available for explicit retry.
    result?;
    handle.close()
}
fn finish_activation(
    state: &mut Option<[u8; 128]>,
    desired: &[u8; 128],
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
    // SAFETY: The usize allocation meets FILE_RENAME_INFO alignment and includes its
    // full trailing UTF-16 name. The checked local absolute path is bounded32767 units.
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
