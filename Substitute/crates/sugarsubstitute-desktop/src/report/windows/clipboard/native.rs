// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Perform the clipboard boundary's precise Win32 ownership transfers.
use super::{Boundary, ReportSurfaceError};
use windows::{
    Win32::{
        Foundation::{
            ERROR_SUCCESS, GetLastError, GlobalFree, HANDLE, HGLOBAL, HWND, SetLastError,
        },
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
    },
    core::Error,
};
pub(in super::super) struct Native {
    pub(in super::super) parent: HWND,
}
impl Boundary for Native {
    fn allocate(&mut self, bytes: usize) -> Result<HGLOBAL, ReportSurfaceError> {
        // SAFETY: Owner charged bounded bytes before this movable allocation.
        unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) }
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_allocate", cause))
    }
    fn write(&mut self, global: HGLOBAL, units: &[u16]) -> Result<(), ReportSurfaceError> {
        // SAFETY: Exact owned block contains the previously admitted payload size.
        let pointer = unsafe { GlobalLock(global) };
        if pointer.is_null() {
            return Err(ReportSurfaceError::new("report_clipboard_lock", Error::from_thread()));
        }
        // SAFETY: Synchronous copy into successful exact-sized lock; slice remains live.
        unsafe {
            std::ptr::copy_nonoverlapping(units.as_ptr(), pointer.cast(), units.len());
        }
        Ok(())
    }
    fn unlock(&mut self, global: HGLOBAL) -> Result<(), ReportSurfaceError> {
        // SAFETY: This owner successfully locked this exact block once.
        // The final successful unlock returns zero with NO_ERROR, rather than BOOL success.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            let result = GlobalUnlock(global);
            let code = GetLastError();
            if result.is_ok() || code == ERROR_SUCCESS {
                Ok(())
            } else {
                result.map_err(|cause| ReportSurfaceError::new("report_clipboard_unlock", cause))
            }
        }
    }
    fn open(&mut self) -> Result<(), ReportSurfaceError> {
        // SAFETY: Exact retained parent lives on this creating UI thread.
        unsafe { OpenClipboard(Some(self.parent)) }
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_open", cause))
    }
    fn empty(&mut self) -> Result<(), ReportSurfaceError> {
        // SAFETY: This thread owns successful OpenClipboard until checked CloseClipboard.
        unsafe { EmptyClipboard() }
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_empty", cause))
    }
    fn transfer(&mut self, global: HGLOBAL) -> Result<HGLOBAL, ReportSurfaceError> {
        // SAFETY: CF_UNICODETEXT receives the owned unlocked movable terminated block.
        unsafe { SetClipboardData(13, Some(HANDLE(global.0))) }
            .map(|handle| HGLOBAL(handle.0))
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_transfer", cause))
    }
    fn close(&mut self) -> Result<(), ReportSurfaceError> {
        // SAFETY: Close this thread's successful open exactly once.
        unsafe { CloseClipboard() }
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_close", cause))
    }
    fn free(&mut self, global: HGLOBAL) -> Result<(), ReportSurfaceError> {
        // SAFETY: Only untransferred unlocked memory remains owned by this native boundary.
        let remaining = unsafe { GlobalFree(Some(global)) }
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_free", cause))?;
        if !remaining.is_invalid() {
            return Err(ReportSurfaceError::rejected("report_clipboard_free_identity"));
        }
        Ok(())
    }
}
