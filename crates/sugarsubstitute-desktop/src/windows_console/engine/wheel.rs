// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Translate wheel input into native document scrolling using Windows' user preference.
use super::Engine;
use crate::DesktopError;
use std::cell::Cell;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::{
        Controls::{EM_GETLINECOUNT, EM_LINESCROLL},
        WindowsAndMessaging::*,
    },
};

/// Preserve sub-notch input between messages without creating timers or changing selection.
#[derive(Default)]
pub(super) struct Wheel {
    remainder: Cell<i32>,
}
impl Wheel {
    /// The windowless engine has no stock window procedure to translate wheel gestures.
    pub(super) fn message(&self, engine: &Engine, wparam: WPARAM) -> Result<bool, DesktopError> {
        // Shift and Control retain the text engine's horizontal/zoom shortcut semantics.
        if wparam.0 & 0x0c != 0 {
            self.remainder.set(0);
            return Ok(false);
        }
        let mut lines = 0_u32;
        // SAFETY: Read the current user setting into one owned scalar; no preference is modified.
        unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                Some((&raw mut lines).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .map_err(|cause| DesktopError::new("console_wheel_preference", cause))?;
        let delta = (wparam.0 >> 16) as u16 as i16 as i32;
        let notches = self.notches(delta);
        if notches == 0 || lines == 0 {
            return Ok(true);
        }
        if lines == u32::MAX {
            // WHEEL_PAGESCROLL in the native SPI contract.
            let command = if notches > 0 { SB_PAGEUP } else { SB_PAGEDOWN };
            for _ in 0..notches.unsigned_abs() {
                engine.message(WM_VSCROLL, WPARAM(command.0 as usize), LPARAM(0))?;
            }
        } else {
            let extent = engine.message(EM_GETLINECOUNT, WPARAM(0), LPARAM(0))?.0.max(0) as i64;
            let amount = (-i64::from(notches) * i64::from(lines)).clamp(-extent, extent);
            engine.message(EM_LINESCROLL, WPARAM(0), LPARAM(amount as isize))?;
        }
        engine.state.invalidate();
        Ok(true)
    }
    /// Accumulate signed high-resolution movement; opposite input cancels retained fractions.
    fn notches(&self, delta: i32) -> i32 {
        let total = self.remainder.get() + delta;
        self.remainder.set(total % WHEEL_DELTA as i32);
        total / WHEEL_DELTA as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_wheel_motion_accumulates_and_reverses_without_losing_notches() {
        let wheel = Wheel::default();
        assert_eq!(wheel.notches(30), 0);
        assert_eq!(wheel.notches(30), 0);
        assert_eq!(wheel.notches(60), 1);
        assert_eq!(wheel.notches(-60), 0);
        assert_eq!(wheel.notches(60), 0);
        assert_eq!(wheel.notches(-240), -2);
        assert_eq!(wheel.notches(120), 1);
    }
}
