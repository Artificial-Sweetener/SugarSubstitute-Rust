// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own scrollbar gestures and native UI timers while the original text engine owns every position.
use super::{
    Engine,
    scrollbar::{self, Model, Part},
    transaction::Scroll,
};
use crate::DesktopError;
use std::cell::Cell;
use windows::Win32::{
    Foundation::{LPARAM, POINT, WPARAM},
    System::SystemInformation::GetTickCount64,
    UI::{Controls::WM_MOUSELEAVE, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const ANIMATION: usize = usize::MAX - 1;
const REPEAT: usize = usize::MAX - 2;
const DURATION: u64 = 100;

/// One native capture and two disjoint, checked timers belong to this exact input registration.
pub(super) struct ScrollInput {
    pressed: Cell<Part>,
    captured: Cell<bool>,
    grab: Cell<i32>,
    point: Cell<POINT>,
    tracking: Cell<bool>,
    expanded: Cell<bool>,
    from: Cell<u8>,
    target: Cell<u8>,
    started: Cell<u64>,
    repeating: Cell<bool>,
    wheel: super::wheel::Wheel,
}
impl ScrollInput {
    /// Construct inert interaction state without installing a timer or capturing input.
    pub(super) fn new() -> Self {
        Self {
            pressed: Cell::new(Part::None),
            captured: Cell::new(false),
            grab: Cell::new(0),
            point: Cell::new(POINT::default()),
            tracking: Cell::new(false),
            expanded: Cell::new(false),
            from: Cell::new(0),
            target: Cell::new(0),
            started: Cell::new(0),
            repeating: Cell::new(false),
            wheel: super::wheel::Wheel::default(),
        }
    }
    /// Obtain physical scrollbar facts from the same native document used for input and drawing.
    pub(super) fn model(&self, engine: &Engine) -> Result<Option<Model>, DesktopError> {
        if engine.state.closing.get() || !engine.state.scroll_visible.get() {
            return Ok(None);
        }
        let native = Scroll::observe(&engine.services()?)?;
        let maximum = native.maximum.saturating_sub(native.page).max(native.minimum);
        if maximum == native.minimum {
            return Ok(None);
        }
        // SAFETY: This monotonic OS scalar drives only this UI adapter's bounded visual animation.
        let now = unsafe { GetTickCount64() };
        Ok(Some(Model {
            bounds: engine.state.bounds.get(),
            scale: engine.state.scale.get(),
            minimum: native.minimum,
            maximum,
            page: native.page as u32,
            position: native.position.clamp(native.minimum, maximum),
            dark: engine.state.system_dark.get(),
            enabled: native.enabled,
            hover: progress(
                self.from.get(),
                self.target.get(),
                now.saturating_sub(self.started.get()),
            ),
            pressed: self.pressed.get(),
        }))
    }
    /// Return true only for messages consumed by this scrollbar rather than native text selection.
    pub(super) fn message(
        &self,
        engine: &Engine,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<bool, DesktopError> {
        match message {
            WM_MOUSEWHEEL => return self.wheel.message(engine, wparam),
            WM_TIMER if wparam.0 == ANIMATION && engine.state.owns_timer(wparam.0) => {
                self.animate(engine)?;
                return Ok(true);
            }
            WM_TIMER if wparam.0 == REPEAT && engine.state.owns_timer(wparam.0) => {
                self.repeat(engine)?;
                return Ok(true);
            }
            WM_CAPTURECHANGED => {
                if self.captured.get() {
                    self.cancel(engine, false)?;
                }
                return Ok(false);
            }
            WM_CANCELMODE | WM_KILLFOCUS => {
                self.cancel(engine, true)?;
                return Ok(false);
            }
            WM_ENABLE | WM_SHOWWINDOW if wparam.0 == 0 => {
                self.cancel(engine, true)?;
                return Ok(false);
            }
            WM_MOUSELEAVE => {
                self.tracking.set(false);
                self.hover(engine, self.pressed.get() != Part::None)?;
                return Ok(false);
            }
            WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_LBUTTONUP => {}
            _ => return Ok(false),
        }
        let point = POINT { x: lparam.0 as i16 as i32, y: (lparam.0 >> 16) as i16 as i32 };
        self.point.set(point);
        let Some(model) = self.model(engine)? else {
            if self.captured.get() {
                self.cancel(engine, true)?;
                return Ok(true);
            }
            return Ok(false);
        };
        let geometry = scrollbar::geometry(&model)?;
        let over = point.x >= geometry.track.left
            && point.x < model.bounds.right
            && point.y >= model.bounds.top
            && point.y < model.bounds.bottom;
        if message == WM_MOUSEMOVE {
            if !self.tracking.get() {
                let mut tracking = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: engine.state.hwnd,
                    dwHoverTime: 0,
                };
                // SAFETY: Track leave notifications only for this original native input child.
                unsafe { TrackMouseEvent(&mut tracking) }
                    .map_err(|cause| DesktopError::new("console_scroll_track", cause))?;
                self.tracking.set(true);
            }
            self.hover(engine, over || self.pressed.get() != Part::None)?;
            if self.pressed.get() == Part::Thumb {
                let position = scrollbar::drag_position(&model, point.y, self.grab.get())?;
                self.position(engine, position)?;
            }
            return Ok(over || self.captured.get());
        }
        if message == WM_LBUTTONUP && self.captured.get() {
            self.cancel(engine, true)?;
            self.hover(engine, over)?;
            return Ok(true);
        }
        if message == WM_LBUTTONDOWN || message == WM_LBUTTONDBLCLK {
            if !over {
                return Ok(false);
            }
            let part = scrollbar::hit_test(&model, point)?;
            if part == Part::None {
                return Ok(true);
            }
            // SAFETY: A real gesture captures only this original input child; identity is checked.
            unsafe {
                SetCapture(engine.state.hwnd);
            }
            // SAFETY: Observe capture on this same UI thread before accepting the gesture.
            if unsafe { GetCapture() } != engine.state.hwnd {
                return Err(DesktopError::message(
                    "console_scroll_capture",
                    "native capture identity differs",
                ));
            }
            self.captured.set(true);
            self.pressed.set(part);
            self.hover(engine, true)?;
            if part == Part::Thumb {
                self.grab.set(point.y - geometry.thumb.top);
            } else {
                self.step(engine, part)?;
                self.repeating.set(false);
                self.timer(engine, REPEAT, 500)?;
            }
            engine.state.invalidate();
            return Ok(true);
        }
        Ok(false)
    }
    /// Cancel only this gesture's capture/timers; preserve failed native obligations for retry.
    pub(super) fn cancel(&self, engine: &Engine, release: bool) -> Result<(), DesktopError> {
        let mut failure = None;
        for id in [ANIMATION, REPEAT] {
            if let Err(cause) = engine.state.kill_timer(id) {
                DesktopError::retain_first(
                    &mut failure,
                    DesktopError::new("console_scroll_timer_retire", cause),
                );
            }
        }
        if self.captured.get() {
            // SAFETY: Capture identity is observed before releasing any native input ownership.
            if release && unsafe { GetCapture() } == engine.state.hwnd {
                // SAFETY: Only this owner's original successful capture is released.
                if let Err(cause) = unsafe { ReleaseCapture() } {
                    DesktopError::retain_first(
                        &mut failure,
                        DesktopError::new("console_scroll_capture_retire", cause),
                    );
                }
            }
            // SAFETY: Keep custody if native capture removal failed; never release another window.
            self.captured.set(unsafe { GetCapture() } == engine.state.hwnd && release);
        }
        self.pressed.set(Part::None);
        self.repeating.set(false);
        self.from.set(0);
        self.target.set(0);
        self.expanded.set(false);
        engine.state.invalidate();
        failure.map_or(Ok(()), Err)
    }
    fn hover(&self, engine: &Engine, expanded: bool) -> Result<(), DesktopError> {
        if self.expanded.replace(expanded) == expanded {
            return Ok(());
        }
        // SAFETY: This monotonic OS scalar never schedules work outside the existing native UI loop.
        let now = unsafe { GetTickCount64() };
        self.from.set(progress(
            self.from.get(),
            self.target.get(),
            now.saturating_sub(self.started.get()),
        ));
        self.target.set(if expanded { 255 } else { 0 });
        self.started.set(now);
        self.timer(engine, ANIMATION, 16)?;
        engine.state.invalidate();
        Ok(())
    }
    fn timer(&self, engine: &Engine, id: usize, period: u32) -> Result<(), DesktopError> {
        if !engine.state.set_timer(id, period).as_bool() {
            return engine
                .state
                .check()
                .and(Err(DesktopError::message("console_scroll_timer", "native timer rejected")));
        }
        Ok(())
    }
    fn animate(&self, engine: &Engine) -> Result<(), DesktopError> {
        // SAFETY: The native timer samples this adapter's monotonic animation endpoint.
        if unsafe { GetTickCount64() }.saturating_sub(self.started.get()) >= DURATION {
            engine
                .state
                .kill_timer(ANIMATION)
                .map_err(|cause| DesktopError::new("console_scroll_animation_retire", cause))?;
            self.from.set(self.target.get());
        }
        engine.state.invalidate();
        Ok(())
    }
    fn repeat(&self, engine: &Engine) -> Result<(), DesktopError> {
        if self.pressed.get() == Part::None {
            return self.cancel(engine, true);
        }
        if !self.repeating.replace(true) {
            self.timer(engine, REPEAT, 50)?;
        }
        if let Some(model) = self.model(engine)?
            && scrollbar::hit_test(&model, self.point.get())? == self.pressed.get()
        {
            self.step(engine, self.pressed.get())?;
        }
        Ok(())
    }
    fn step(&self, engine: &Engine, part: Part) -> Result<(), DesktopError> {
        let command = match part {
            Part::Decrement => SB_LINEUP,
            Part::Increment => SB_LINEDOWN,
            Part::PageBefore => SB_PAGEUP,
            Part::PageAfter => SB_PAGEDOWN,
            _ => return Ok(()),
        };
        engine.message(WM_VSCROLL, WPARAM(command.0 as usize), LPARAM(0))?;
        engine.state.invalidate();
        Ok(())
    }
    fn position(&self, engine: &Engine, position: i32) -> Result<(), DesktopError> {
        use windows::Win32::UI::Controls::RichEdit::{EM_GETSCROLLPOS, EM_SETSCROLLPOS};
        let mut point = POINT::default();
        engine.message(EM_GETSCROLLPOS, WPARAM(0), LPARAM((&raw mut point) as isize))?;
        point.y = position;
        engine.message(EM_SETSCROLLPOS, WPARAM(0), LPARAM((&raw const point) as isize))?;
        engine.state.invalidate();
        Ok(())
    }
}
/// Qt's numeric hover animation is linear, exact at both endpoints and reversible mid-flight.
fn progress(from: u8, to: u8, elapsed: u64) -> u8 {
    let elapsed = elapsed.min(DURATION) as i64;
    (i64::from(from) + (i64::from(to) - i64::from(from)) * elapsed / DURATION as i64) as u8
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hover_endpoints_reverse_from_current_value_without_wall_clock_waits() {
        assert_eq!(progress(0, 255, 0), 0);
        assert_eq!(progress(0, 255, 50), 127);
        assert_eq!(progress(0, 255, 100), 255);
        assert_eq!(progress(0, 255, u64::MAX), 255);
        assert_eq!(progress(127, 0, 0), 127);
        assert_eq!(progress(127, 0, 100), 0);
        assert_eq!(progress(255, 0, 50), 128);
        const {
            assert!(usize::BITS >= 64);
            assert!(ANIMATION > u32::MAX as usize);
            assert!(REPEAT > u32::MAX as usize);
        }
        assert_ne!(ANIMATION, REPEAT);
    }
}
