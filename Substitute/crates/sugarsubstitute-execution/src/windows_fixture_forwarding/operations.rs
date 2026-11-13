// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! External handle operations; adoption and partial cleanup remain with Lease.
use super::*;
pub(super) trait Operations {
    fn flags(&mut self, raw: usize) -> Result<u32, NativeObservationError>;
    fn clear_inherit(&mut self, raw: usize) -> Result<(), NativeObservationError>;
    fn validate_event(&mut self, raw: usize) -> Result<(), NativeObservationError>;
    fn duplicate(&mut self, raw: usize, output: &mut usize) -> Result<(), NativeObservationError>;
    fn close(&mut self, raw: usize) -> Result<(), NativeObservationError>;
}
pub(super) struct Native;
impl Operations for Native {
    fn flags(&mut self, raw: usize) -> Result<u32, NativeObservationError> {
        let mut flags = 0;
        // SAFETY: Lease retains this exact original or adopted duplicate throughout the query.
        unsafe { GetHandleInformation(HANDLE(raw as *mut _), &mut flags) }
            .map_err(|cause| NativeObservationError::new("forwarding_handle_flags", cause))?;
        Ok(flags)
    }
    fn clear_inherit(&mut self, raw: usize) -> Result<(), NativeObservationError> {
        // SAFETY: only the exclusively adopted original's inherit flag changes.
        unsafe {
            SetHandleInformation(HANDLE(raw as *mut _), HANDLE_FLAG_INHERIT.0, Default::default())
        }
        .map_err(|cause| NativeObservationError::new("forwarding_clear_inherit", cause))
    }
    fn validate_event(&mut self, raw: usize) -> Result<(), NativeObservationError> {
        // SAFETY: validates actual event type and rights before descendant readiness.
        // The admitted fixture roles start false; no parent command precedes Ready.
        unsafe { ResetEvent(HANDLE(raw as *mut _)) }
            .map_err(|cause| NativeObservationError::new("forwarding_validate_event", cause))
    }
    fn duplicate(&mut self, raw: usize, output: &mut usize) -> Result<(), NativeObservationError> {
        let mut duplicate = HANDLE::default();
        // SAFETY: source remains retained; caller adopts every actual output before checking result.
        let result = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                HANDLE(raw as *mut _),
                GetCurrentProcess(),
                &mut duplicate,
                EVENT_MODIFY_STATE.0 | SYNCHRONIZATION_SYNCHRONIZE.0,
                true,
                Default::default(),
            )
        };
        if !duplicate.is_invalid() {
            *output = duplicate.0 as usize;
        }
        result.map_err(|cause| NativeObservationError::new("forwarding_duplicate", cause))
    }
    fn close(&mut self, raw: usize) -> Result<(), NativeObservationError> {
        // SAFETY: pin is absent; caller retains the exact unique slot on failure.
        unsafe { CloseHandle(HANDLE(raw as *mut _)) }
            .map_err(|cause| NativeObservationError::new("forwarding_close", cause))
    }
}
