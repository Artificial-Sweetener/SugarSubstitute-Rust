// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read protected fixed bytes through retained actual source custody.
use super::*;
pub(super) fn shape<const N: usize>(
    file: &HeldFile<N>,
    expected: Option<[u8; 24]>,
    check: &mut dyn FnMut() -> Result<(), ClaimError>,
) -> Result<RecordRead<N>, ClaimError> {
    let path = paths::wide(&file.record_path)?;
    let mut handles = lock(&file.temporary_handles);
    if handles.len() >= 16 || !(16..=1024).contains(&N) {
        return Err(ClaimError::invalid("temporary_handle_bound"));
    }
    handles
        .try_reserve_exact(1)
        .map_err(|cause| ClaimError::new("record_handle_reservation", cause))?;
    check()?;
    // SAFETY: The bounded owned NUL path remains live through read-only OPEN_EXISTING.
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
    check()?;
    let handle = &mut handles[index];
    let observed = (|| {
        check()?;
        paths::validate_regular(handle, None)?;
        check()?;
        security::verify(handle, &file.account_sid)?;
        check()?;
        if paths::final_path(handle)? != file.record_path {
            return Err(ClaimError::invalid("changed_record_path"));
        }
        if let Some(identity) = expected
            && paths::file_identity(handle)? != identity
        {
            return Err(ClaimError::invalid("changed_retirement_record_identity"));
        }
        let mut length = 0_i64;
        check()?;
        // SAFETY: Retained source and initialized size output stay live synchronously.
        unsafe { GetFileSizeEx(handle.raw(), &mut length) }
            .map_err(|cause| ClaimError::new("claim_record_shape", cause))?;
        check()?;
        if length == 0 {
            return Ok(RecordRead::Empty);
        }
        if length != N as i64 {
            return Err(ClaimError::invalid("claim_file_shape"));
        }
        let mut bytes = [0; N];
        let mut count = 0;
        check()?;
        // SAFETY: Exact initialized bounded output and synchronous source remain owned.
        unsafe { ReadFile(handle.raw(), Some(&mut bytes), Some(&mut count), None) }
            .map_err(|cause| ClaimError::new("read_claim_record", cause))?;
        check()?;
        if count as usize != N {
            return Err(ClaimError::invalid("short_claim_record"));
        }
        Ok(RecordRead::Present(bytes))
    })();
    let cleanup = check().and_then(|()| handle.close());
    let cleanup = ClaimError::finish(cleanup, check());
    if cleanup.is_ok() {
        handles.remove(index);
    }
    ClaimError::finish(observed, cleanup)
}
pub(super) fn activated<const N: usize>(
    handle: &Handle,
    expected: &[u8; N],
    check: &mut dyn FnMut() -> Result<(), ClaimError>,
) -> Result<(), ClaimError> {
    check()?;
    paths::validate_regular(handle, Some(N as i64))?;
    check()?;
    let mut bytes = [0; N];
    let mut read = 0;
    // SAFETY: Actual retained stage is borrowed throughout exact synchronous seek/read.
    unsafe {
        SetFilePointerEx(handle.raw(), 0, None, FILE_BEGIN)
            .map_err(|cause| ClaimError::new("retirement_seek", cause))?;
        check()?;
        ReadFile(handle.raw(), Some(&mut bytes), Some(&mut read), None)
            .map_err(|cause| ClaimError::new("retirement_read", cause))?;
    }
    check()?;
    if read as usize != N || &bytes != expected {
        return Err(ClaimError::invalid("changed_retained_activation"));
    }
    Ok(())
}
