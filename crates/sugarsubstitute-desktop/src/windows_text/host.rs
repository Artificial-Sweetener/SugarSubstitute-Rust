// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain affine window facts and every native timer/DC obligation for one RichEdit host.
use crate::DesktopError;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use sugarsubstitute_ui::windows::text_style::TextStyle;
use windows::{
    Win32::{
        Foundation::{E_INVALIDARG, HWND, RECT},
        Graphics::Gdi::{GetDC, HDC, ReleaseDC},
        UI::WindowsAndMessaging::{GetClientRect, KillTimer, SetTimer},
    },
    core::{BOOL, Error, Result},
};

/// Shared native state never owns or duplicates the document; the outer peer owns its HWND.
pub(crate) struct State {
    pub(crate) hwnd: HWND,
    pub(crate) style: TextStyle,
    pub(crate) revision: Cell<u64>,
    pub(crate) bounds: Cell<RECT>,
    pub(crate) scale: Cell<f64>,
    pub(crate) system_dark: Cell<bool>,
    pub(crate) closing: Cell<bool>,
    pub(crate) painting: Rc<Cell<bool>>,
    pub(crate) observing_blocked: Rc<Cell<bool>>,
    pub(crate) redraw: Cell<bool>,
    pub(crate) dirty: Cell<bool>,
    pub(crate) active: Cell<i32>,
    pub(crate) ui_active: Cell<bool>,
    pub(crate) scroll_visible: Cell<bool>,
    pub(crate) scroll_min: Cell<i32>,
    pub(crate) scroll_max: Cell<i32>,
    pub(crate) scroll_position: Cell<i32>,
    pub(crate) caret_position: Cell<(i32, i32)>,
    pub(crate) caret_extent: Cell<(i32, i32)>,
    pub(crate) caret_visible: Cell<bool>,
    pub(crate) caret: sugarsubstitute_ui::windows::caret::Caret,
    pub(crate) default_foreground: Cell<u32>,
    pub(crate) selection_background: Cell<u32>,
    pub(crate) selection_foreground: Cell<u32>,
    pub(crate) font: Cell<windows::Win32::UI::Controls::RichEdit::CHARFORMAT2W>,
    pub(crate) paragraph: Cell<windows::Win32::UI::Controls::RichEdit::PARAFORMAT>,
    dcs: Cell<[HDC; 8]>,
    timers: Cell<[usize; 16]>,
    failure: RefCell<Option<DesktopError>>,
    failed: Cell<bool>,
}
impl State {
    /// Construct inert callback state before creating any native text-services resource.
    pub(crate) fn new(hwnd: HWND, style: TextStyle) -> Self {
        use windows::Win32::UI::Controls::RichEdit::*;
        let font = style.font();
        let paragraph = PARAFORMAT {
            cbSize: size_of::<PARAFORMAT>() as u32,
            dwMask: PFM_ALIGNMENT,
            wAlignment: PFA_LEFT,
            ..Default::default()
        };
        Self {
            hwnd,
            style,
            revision: Cell::new(0),
            bounds: Cell::new(RECT::default()),
            scale: Cell::new(1.0),
            system_dark: Cell::new(false),
            closing: Cell::new(false),
            painting: Rc::new(Cell::new(false)),
            observing_blocked: Rc::new(Cell::new(false)),
            redraw: Cell::new(true),
            dirty: Cell::new(true),
            active: Cell::new(0),
            ui_active: Cell::new(false),
            scroll_visible: Cell::new(false),
            scroll_min: Cell::new(0),
            scroll_max: Cell::new(0),
            scroll_position: Cell::new(0),
            caret_position: Cell::new((0, 0)),
            caret_extent: Cell::new((0, 0)),
            caret_visible: Cell::new(false),
            caret: sugarsubstitute_ui::windows::caret::Caret::new(hwnd),
            default_foreground: Cell::new(0),
            selection_background: Cell::new(0),
            selection_foreground: Cell::new(0),
            font: Cell::new(font),
            paragraph: Cell::new(paragraph),
            dcs: Cell::new([HDC::default(); 8]),
            timers: Cell::new([0; 16]),
            failure: RefCell::new(None),
            failed: Cell::new(false),
        }
    }
    /// Preserve the first unexpected native callback cause for its containing owner.
    pub(crate) fn fail(
        &self,
        operation: &'static str,
        cause: impl std::error::Error + Send + Sync + 'static,
    ) {
        self.failed.set(true);
        DesktopError::retain_first(
            &mut self.failure.borrow_mut(),
            DesktopError::new(operation, cause),
        );
    }
    /// Preserve an already typed failure from the native message adapter.
    pub(crate) fn record(&self, cause: DesktopError) {
        self.failed.set(true);
        DesktopError::retain_first(&mut self.failure.borrow_mut(), cause);
    }
    /// Accept WM_TIMER only for one still-owned native callback obligation.
    pub(crate) fn owns_timer(&self, id: usize) -> bool {
        id != 0 && self.timers.get().contains(&id)
    }
    /// Keep the transparent custom scrollbar outside the native text viewport.
    pub(crate) fn client_bounds(&self) -> RECT {
        let mut bounds = self.bounds.get();
        if self.scroll_visible.get() {
            bounds.right =
                (bounds.right - (12.0 * self.scale.get()).round() as i32).max(bounds.left);
        }
        bounds
    }
    /// Return the original cause once and reject further effects after any observed failure.
    pub(crate) fn check(&self) -> std::result::Result<(), DesktopError> {
        if let Some(cause) = self.failure.borrow_mut().take() {
            return Err(cause);
        }
        if self.failed.get() {
            return Err(DesktopError::message(
                "console_host_failed",
                "native host previously failed",
            ));
        }
        Ok(())
    }
    /// Query the live child rectangle without involving another window or desktop.
    pub(crate) fn refresh_bounds(&self) -> std::result::Result<(), DesktopError> {
        let mut bounds = RECT::default();
        // SAFETY: The outer peer retains this affine child and this writable output.
        unsafe { GetClientRect(self.hwnd, &mut bounds) }
            .map_err(|cause| DesktopError::new("console_host_bounds", cause))?;
        self.bounds.set(bounds);
        self.dirty.set(true);
        Ok(())
    }
    /// Invalidate only the exact child; deferring during paint prevents exposing staged formats.
    pub(crate) fn invalidate(&self) {
        self.dirty.set(true);
        self.expose();
    }
    /// Refresh the composed background without invalidating unchanged native text and selection.
    pub(crate) fn expose(&self) {
        if self.closing.get() || self.painting.get() || !self.redraw.get() {
            return;
        }
        // SAFETY: Invalidate this retained child without erasing or recursively pumping messages.
        let invalidated =
            unsafe { windows::Win32::Graphics::Gdi::InvalidateRect(Some(self.hwnd), None, false) }
                .ok();
        if let Err(cause) = invalidated {
            self.fail("console_host_invalidate", cause);
        }
    }
    /// Track each borrowed window DC until its matching native callback releases it.
    pub(crate) fn dc(&self) -> HDC {
        let mut held = self.dcs.get();
        let Some(index) = held.iter().position(HDC::is_invalid) else {
            self.fail("console_host_dc_limit", Error::from(E_INVALIDARG));
            return HDC::default();
        };
        // SAFETY: The outer peer owns the original live affine HWND.
        let dc = unsafe { GetDC(Some(self.hwnd)) };
        if dc.is_invalid() {
            self.fail("console_host_get_dc", Error::from_thread());
        } else {
            held[index] = dc;
            self.dcs.set(held);
        }
        dc
    }
    /// Release exactly one owned matching native DC reference.
    pub(crate) fn release_dc(&self, dc: HDC) -> i32 {
        let mut held = self.dcs.get();
        let index = held.iter().position(|held| *held == dc);
        let Some(index) = index else {
            self.fail("console_host_dc_identity", Error::from(E_INVALIDARG));
            return 0;
        };
        // SAFETY: This exact HWND/DC pair came from the checked acquisition above.
        let released = unsafe { ReleaseDC(Some(self.hwnd), dc) };
        if released == 0 {
            self.fail("console_host_release_dc", Error::from_thread());
        } else {
            held[index] = HDC::default();
            self.dcs.set(held);
        }
        released
    }
    /// Capture only this owned text input HWND and verify the native result immediately.
    pub(crate) fn capture(&self) -> std::result::Result<(), DesktopError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, SetCapture};
        // SAFETY: Only this original retained child captures a real text-input gesture.
        unsafe {
            SetCapture(self.hwnd);
        }
        // SAFETY: Capture is a passive thread-affine identity observation.
        if unsafe { GetCapture() } != self.hwnd {
            return Err(DesktopError::message(
                "console_text_capture",
                "native capture identity differs",
            ));
        }
        Ok(())
    }
    /// Release only actual capture belonging to this same original native child.
    pub(crate) fn release_capture(&self) -> std::result::Result<(), DesktopError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture};
        // SAFETY: Observe capture ownership before any release; other windows remain untouched.
        if unsafe { GetCapture() } == self.hwnd {
            // SAFETY: The native capture identity is this exact retained input child.
            unsafe { ReleaseCapture() }
                .map_err(|cause| DesktopError::new("console_text_capture_release", cause))?;
            // SAFETY: Retain the obligation if native release failed to retire its identity.
            if unsafe { GetCapture() } == self.hwnd {
                return Err(DesktopError::message(
                    "console_text_capture_release",
                    "native capture remains owned",
                ));
            }
        }
        Ok(())
    }
    /// Native event-loop timers stay scoped to the child and are bounded independently of IDs.
    pub(crate) fn set_timer(&self, id: usize, period_ms: u32) -> BOOL {
        let mut timers = self.timers.get();
        let index = timers
            .iter()
            .position(|held| *held == id)
            .or_else(|| timers.iter().position(|held| *held == 0));
        let Some(index) = index.filter(|_| !self.closing.get() && id != 0 && period_ms != 0) else {
            self.fail("console_host_timer_admission", Error::from(E_INVALIDARG));
            return false.into();
        };
        // SAFETY: WM_TIMER is delivered on this child's existing native UI event loop.
        let installed = unsafe { SetTimer(Some(self.hwnd), id, period_ms, None) };
        if installed == 0 {
            self.fail("console_host_timer_install", Error::from_thread());
            return false.into();
        }
        timers[index] = installed;
        self.timers.set(timers);
        true.into()
    }
    /// Cancelling an absent timer is idempotent; installed timers require checked native removal.
    pub(crate) fn kill_timer(&self, id: usize) -> Result<()> {
        let mut timers = self.timers.get();
        let Some(index) = timers.iter().position(|held| *held == id && *held != 0) else {
            return Ok(());
        };
        // SAFETY: Only timer identities successfully installed for this original child are removed.
        unsafe { KillTimer(Some(self.hwnd), id) }?;
        timers[index] = 0;
        self.timers.set(timers);
        Ok(())
    }
    /// Retire all actual timer handles before the text host or HWND can be destroyed.
    pub(crate) fn stop_timers(&self) -> std::result::Result<(), DesktopError> {
        loop {
            let id = self.timers.get().into_iter().find(|id| *id != 0);
            let Some(id) = id else {
                break;
            };
            self.kill_timer(id)
                .map_err(|cause| DesktopError::new("console_host_timer_retire", cause))?;
        }
        Ok(())
    }
    /// No native resource or callback may survive its containing child/document retirement.
    pub(crate) fn retired(&self) -> std::result::Result<(), DesktopError> {
        if self.timers.get().iter().any(|id| *id != 0)
            || self.dcs.get().iter().any(|dc| !dc.is_invalid())
            || self.painting.get()
        {
            return Err(DesktopError::message(
                "console_host_retirement",
                "native timer, DC or paint obligation remains",
            ));
        }
        Ok(())
    }
}
