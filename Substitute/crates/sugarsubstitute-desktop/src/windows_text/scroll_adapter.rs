// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt the original RichEdit document and timer ledger to shared scrollbar input.
use super::{Engine, scroll::Scroll};
use sugarsubstitute_ui::windows::{
    NativeUiError,
    scroll_port::{ScrollHost, ScrollStep},
    scrollbar::{Model, Part},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    System::SystemInformation::GetTickCount64,
    UI::{
        Controls::{
            EM_GETLINECOUNT, EM_LINESCROLL,
            RichEdit::{EM_GETSCROLLPOS, EM_SETSCROLLPOS},
        },
        WindowsAndMessaging::*,
    },
};

impl ScrollHost for Engine {
    fn window(&self) -> HWND {
        self.state.hwnd
    }
    fn now_millis(&self) -> u64 {
        // SAFETY: This OS scalar observes time without allocating or scheduling work.
        unsafe { GetTickCount64() }
    }
    fn observe(&self, hover: u8, pressed: Part) -> Result<Option<Model>, NativeUiError> {
        if self.state.closing.get() || !self.state.scroll_visible.get() {
            return Ok(None);
        }
        let native = Scroll::observe(
            &self.services().map_err(|e| NativeUiError::new("console_scroll_services", e))?,
        )
        .map_err(|e| NativeUiError::new("console_scroll_observe", e))?;
        let maximum = native.maximum.saturating_sub(native.page).max(native.minimum);
        if maximum == native.minimum {
            return Ok(None);
        }
        Ok(Some(Model {
            appearance: sugarsubstitute_ui::windows::scrollbar::Appearance::Overlay,
            bounds: self.state.bounds.get(),
            scale: self.state.scale.get(),
            minimum: native.minimum,
            maximum,
            page: native.page as u32,
            position: native.position.clamp(native.minimum, maximum),
            dark: self.state.system_dark.get(),
            enabled: native.enabled,
            hover,
            pressed,
        }))
    }
    fn scroll(&self, step: ScrollStep) -> Result<(), NativeUiError> {
        let command = match step {
            ScrollStep::LineBefore => SB_LINEUP,
            ScrollStep::LineAfter => SB_LINEDOWN,
            ScrollStep::PageBefore => SB_PAGEUP,
            ScrollStep::PageAfter => SB_PAGEDOWN,
        };
        self.message(WM_VSCROLL, WPARAM(command.0 as usize), LPARAM(0))
            .map_err(|e| NativeUiError::new("console_scroll_step", e))?;
        Ok(())
    }
    fn scroll_lines(&self, amount: i64) -> Result<(), NativeUiError> {
        let extent = self
            .message(EM_GETLINECOUNT, WPARAM(0), LPARAM(0))
            .map_err(|e| NativeUiError::new("console_scroll_extent", e))?
            .0
            .max(0) as i64;
        self.message(EM_LINESCROLL, WPARAM(0), LPARAM(amount.clamp(-extent, extent) as isize))
            .map_err(|e| NativeUiError::new("console_scroll_lines", e))?;
        Ok(())
    }
    fn set_position(&self, position: i32) -> Result<(), NativeUiError> {
        let mut point = POINT::default();
        self.message(EM_GETSCROLLPOS, WPARAM(0), LPARAM((&raw mut point) as isize))
            .map_err(|e| NativeUiError::new("console_scroll_position", e))?;
        point.y = position;
        self.message(EM_SETSCROLLPOS, WPARAM(0), LPARAM((&raw const point) as isize))
            .map_err(|e| NativeUiError::new("console_scroll_position", e))?;
        Ok(())
    }
    fn owns_timer(&self, id: usize) -> bool {
        self.state.owns_timer(id)
    }
    fn set_timer(&self, id: usize, milliseconds: u32) -> Result<(), NativeUiError> {
        if !self.state.set_timer(id, milliseconds).as_bool() {
            self.state.check().map_err(|e| NativeUiError::new("console_scroll_timer", e))?;
            return Err(NativeUiError::new(
                "console_scroll_timer",
                std::io::Error::other("Native timer rejected"),
            ));
        }
        Ok(())
    }
    fn kill_timer(&self, id: usize) -> Result<(), NativeUiError> {
        self.state.kill_timer(id).map_err(|e| NativeUiError::new("console_scroll_timer_retire", e))
    }
    fn invalidate(&self) {
        self.state.invalidate();
    }
}
