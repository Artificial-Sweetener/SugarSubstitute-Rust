// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Checked channel handles retain their admission until actual native closure.
use super::*;
use windows::Win32::Foundation::{CloseHandle, HANDLE};

/// One physical slot with the memory admission that remains charged through checked closure.
pub(super) struct Handle {
    raw: usize,
    memory: Arc<MemoryClaim>,
}
impl Handle {
    /// Adopt one validated uniquely owned native output under the original reservation.
    pub(super) fn new(raw: HANDLE, memory: Arc<MemoryClaim>) -> Self {
        Self { raw: raw.0 as usize, memory }
    }
    /// Borrow only while this cell retains its original physical output.
    pub(super) fn raw(&self) -> HANDLE {
        HANDLE(self.raw as *mut _)
    }
    /// A failed checked close remains armed for a subsequent cleanup attempt.
    pub(super) fn is_armed(&self) -> bool {
        self.raw != 0
    }
    /// Clear the physical slot only after Windows reports successful closure.
    pub(super) fn close(&mut self) -> Result<(), NativeObservationError> {
        if self.raw != 0 {
            // SAFETY: this cell uniquely owns the exact handle; errors preserve the slot.
            unsafe { CloseHandle(self.raw()) }
                .map_err(|cause| NativeObservationError::new("activation_channel_close", cause))?;
            self.raw = 0;
        }
        Ok(())
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation="activation_channel_close_retained", cause=%cause, "Failed native channel closure retains admission until process exit");
            std::mem::forget(self.memory.clone());
        }
    }
}
