// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Suspend native presentation and provider observation without changing child availability.
use crate::DesktopError;
use windows::Win32::{
    Foundation::{HWND, WPARAM},
    UI::WindowsAndMessaging::{SendMessageW, WM_SETREDRAW},
};

/// The containing Console retains the original affine child through synchronous publication.
pub(super) struct Suspension {
    hwnd: HWND,
    previous: bool,
}
impl Suspension {
    /// The owned input procedure handles redraw directly; hidden peers keep their original style.
    pub(super) fn new(hwnd: HWND) -> Result<Self, DesktopError> {
        let engine = super::super::engine::find(hwnd)?;
        engine.check()?;
        let previous = engine.redraw_allowed();
        // SAFETY: Only the checked original affine child synchronously receives this scalar state.
        unsafe {
            SendMessageW(hwnd, WM_SETREDRAW, Some(WPARAM(0)), None);
        }
        engine.check()?;
        Ok(Self { hwnd, previous })
    }
}
impl Drop for Suspension {
    /// Restore the previous scope on success, failure or unwinding without adding WS_VISIBLE.
    fn drop(&mut self) {
        // SAFETY: The outer publication retains this exact native child until scope restoration.
        unsafe {
            SendMessageW(self.hwnd, WM_SETREDRAW, Some(WPARAM(usize::from(self.previous))), None);
        }
    }
}
