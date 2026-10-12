// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep every returned handle in a durable slot until checked close actually succeeds.
use super::NativePipeError;
use crate::native::NativeObservationError;
use windows::Win32::Foundation::*;
pub(crate) struct Handle(pub(crate) usize);
impl Handle {
    pub(crate) fn raw(&self) -> HANDLE {
        HANDLE(self.0 as *mut _)
    }
    pub(crate) fn close(&mut self, stage: &'static str) -> Result<(), NativePipeError> {
        if self.0 == 0 {
            return Ok(());
        }
        // SAFETY: this is the unique retained slot; failure leaves it unchanged.
        unsafe { CloseHandle(self.raw()) }.map_err(|e| error(stage, e))?;
        self.0 = 0;
        Ok(())
    }
}
pub(crate) fn adopt(
    slot: &mut Handle,
    raw: HANDLE,
    stage: &'static str,
) -> Result<(), NativePipeError> {
    if raw.is_invalid() {
        return Err(error(stage, last_error()));
    }
    slot.0 = raw.0 as usize;
    Ok(())
}
pub(crate) fn last_error() -> windows::core::Error {
    // SAFETY: read this thread's immediate numeric Win32 error without borrowed storage.
    let code = unsafe { GetLastError() };
    windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(if code.0 == 0 {
        ERROR_GEN_FAILURE.0
    } else {
        code.0
    }))
}
pub(crate) fn code(error: &windows::core::Error, expected: WIN32_ERROR) -> bool {
    error.code() == windows::core::HRESULT::from_win32(expected.0)
}
pub(crate) fn error(
    stage: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> NativePipeError {
    NativePipeError::native(NativeObservationError::new(stage, cause))
}
