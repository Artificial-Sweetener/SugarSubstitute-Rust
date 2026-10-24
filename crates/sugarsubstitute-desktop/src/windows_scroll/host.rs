// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Page-position and affine timer effects behind the shared scrollbar interaction owner.
use super::*;
use sugarsubstitute_ui::windows::{
    NativeUiError,
    scroll_port::{ScrollHost, ScrollStep},
};
use windows::Win32::{
    System::SystemInformation::GetTickCount64,
    UI::WindowsAndMessaging::{KillTimer, SetTimer},
};

impl ScrollHost for State {
    fn window(&self) -> HWND {
        self.hwnd
    }
    fn now_millis(&self) -> u64 {
        // SAFETY: Observing the system's monotonic scalar creates no scheduling ownership.
        unsafe { GetTickCount64() }
    }
    fn observe(&self, hover: u8, pressed: Part) -> Result<Option<Model>, NativeUiError> {
        Ok(self.model.get().filter(|_| self.active.get()).map(|m| Model { hover, pressed, ..m }))
    }
    fn scroll(&self, step: ScrollStep) -> Result<(), NativeUiError> {
        let Some(model) = self.model.get() else {
            return Ok(());
        };
        let distance = match step {
            ScrollStep::LineBefore => -20 * UNITS as i64,
            ScrollStep::LineAfter => 20 * UNITS as i64,
            ScrollStep::PageBefore => -i64::from(model.page),
            ScrollStep::PageAfter => i64::from(model.page),
        };
        self.move_by(distance)
    }
    fn scroll_lines(&self, amount: i64) -> Result<(), NativeUiError> {
        self.move_by(amount.saturating_mul(20 * UNITS as i64))
    }
    fn set_position(&self, position: i32) -> Result<(), NativeUiError> {
        if !self.active.get() {
            return Ok(());
        }
        if let Some(mut model) = self.model.get() {
            let position = position.clamp(model.minimum, model.maximum);
            if position != model.position {
                model.position = position;
                self.model.set(Some(model));
                self.pending.set(Some(position));
                self.dirty.set(true);
            }
        }
        Ok(())
    }
    fn owns_timer(&self, id: usize) -> bool {
        id != 0 && self.timers.get().contains(&id)
    }
    fn set_timer(&self, id: usize, milliseconds: u32) -> Result<(), NativeUiError> {
        let mut held = self.timers.get();
        let slot = held
            .iter()
            .position(|current| *current == id)
            .or_else(|| held.iter().position(|current| *current == 0));
        let Some(slot) = slot.filter(|_| self.active.get() && id != 0 && milliseconds != 0) else {
            return Err(NativeUiError::new(
                "native_scroll_timer",
                std::io::Error::other("Timer admission rejected"),
            ));
        };
        // SAFETY: Retain this original parent's exact timer; delivery uses its existing UI loop.
        let actual = unsafe { SetTimer(Some(self.hwnd), id, milliseconds, None) };
        if actual == 0 {
            return Err(NativeUiError::new(
                "native_scroll_timer",
                windows::core::Error::from_thread(),
            ));
        }
        held[slot] = actual;
        self.timers.set(held);
        Ok(())
    }
    fn kill_timer(&self, id: usize) -> Result<(), NativeUiError> {
        let mut held = self.timers.get();
        if let Some(slot) = held.iter().position(|current| *current != 0 && *current == id) {
            // SAFETY: Remove only a successful timer registration owned by this original window.
            unsafe { KillTimer(Some(self.hwnd), id) }
                .map_err(|e| NativeUiError::new("native_scroll_timer_retire", e))?;
            held[slot] = 0;
            self.timers.set(held);
        }
        Ok(())
    }
    fn invalidate(&self) {
        self.dirty.set(true);
    }
}
impl State {
    /// Clamp before narrowing; large wheel inputs cannot wrap the original page position.
    fn move_by(&self, distance: i64) -> Result<(), NativeUiError> {
        if let Some(model) = self.model.get() {
            self.set_position(
                i64::from(model.position)
                    .saturating_add(distance)
                    .clamp(i64::from(model.minimum), i64::from(model.maximum))
                    as i32,
            )?;
        }
        Ok(())
    }
}
