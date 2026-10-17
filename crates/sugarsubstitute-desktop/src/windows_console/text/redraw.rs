// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep streaming and temporary formatting selections inside one native redraw suspension.
use crate::DesktopError;
use windows::Win32::{
    Foundation::{ERROR_SUCCESS, GetLastError, HWND, SetLastError, WPARAM},
    UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SendMessageW, WM_SETREDRAW, WS_VISIBLE,
    },
};

/// The containing Console retains this exact affine HWND until publication returns.
pub(super) struct Suspension {
    hwnd: HWND,
    suspended: bool,
}
impl Suspension {
    /// Hidden peers remain hidden: enabling redraw can otherwise add WS_VISIBLE.
    pub(super) fn new(hwnd: HWND) -> Result<Self, DesktopError> {
        // SAFETY: The original retained peer belongs to this calling UI thread.
        let (style, code) = unsafe {
            SetLastError(ERROR_SUCCESS);
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            (style, GetLastError())
        };
        if style == 0 && code != ERROR_SUCCESS {
            return Err(DesktopError::new(
                "console_redraw_style",
                windows::core::Error::from(windows::core::HRESULT::from_win32(code.0)),
            ));
        }
        let suspended = style & WS_VISIBLE.0 != 0;
        if suspended {
            // SAFETY: Scalar redraw control is synchronous and has no meaningful return value.
            unsafe {
                SendMessageW(hwnd, WM_SETREDRAW, Some(WPARAM(0)), None);
            }
        }
        Ok(Self { hwnd, suspended })
    }
}
impl Drop for Suspension {
    /// Restore on success, native failure or unwinding before the retained peer can retire.
    fn drop(&mut self) {
        if self.suspended {
            // SAFETY: The outer publication borrow still retains this original affine HWND.
            unsafe {
                SendMessageW(self.hwnd, WM_SETREDRAW, Some(WPARAM(1)), None);
            }
        }
    }
}
