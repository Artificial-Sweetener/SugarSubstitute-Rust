// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared Fluent scrollbar gestures; the native host retains document positions and timers.
use super::scrollbar::{self, Model, Part};
use super::{
    NativeUiError,
    scroll_port::{ScrollHost, ScrollStep},
};
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use windows::Win32::{
    Foundation::{LPARAM, POINT, WPARAM},
    UI::{Controls::WM_MOUSELEAVE, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const ANIMATION: usize = usize::MAX - 1;
const REPEAT: usize = usize::MAX - 2;
const DURATION: u64 = 100;

/// One native capture and two disjoint, checked timers belong to this exact input registration.
pub struct ScrollInput {
    affinity: PhantomData<Rc<()>>,
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
    wheel: super::scroll_wheel::Wheel,
}
impl Default for ScrollInput {
    fn default() -> Self {
        Self::new()
    }
}
impl ScrollInput {
    /// Whether this original widget currently retains a pointer gesture's native capture.
    pub fn owns_capture(&self) -> bool {
        self.captured.get()
    }

    /// Apply viewport keyboard navigation only after the host has admitted the focused input role.
    /// Text selection and selector popup keys must remain with their respective native documents.
    /// # Errors
    /// Preserves document observation and movement failures.
    pub fn key(
        &self,
        host: &impl ScrollHost,
        key: super::scroll_port::ScrollKey,
    ) -> Result<bool, NativeUiError> {
        use super::scroll_port::ScrollKey;
        let Some(model) = self.model(host)? else {
            return Ok(false);
        };
        match key {
            ScrollKey::PageBefore => host.scroll(ScrollStep::PageBefore)?,
            ScrollKey::PageAfter => host.scroll(ScrollStep::PageAfter)?,
            ScrollKey::Start => host.set_position(model.minimum)?,
            ScrollKey::End => host.set_position(model.maximum)?,
        }
        host.invalidate();
        Ok(true)
    }
    /// Construct inert interaction state without installing a timer or capturing input.
    pub fn new() -> Self {
        Self {
            affinity: PhantomData,
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
            wheel: super::scroll_wheel::Wheel::default(),
        }
    }
    /// Obtain physical geometry from the authoritative document and current visual state.
    /// # Errors
    /// Preserves native document observation failures.
    pub fn model(&self, host: &impl ScrollHost) -> Result<Option<Model>, NativeUiError> {
        host.observe(
            progress(
                self.from.get(),
                self.target.get(),
                host.now_millis().saturating_sub(self.started.get()),
            ),
            self.pressed.get(),
        )
    }
    /// Consume only scrollbar messages; text selection remains with the native document.
    /// Call on the owning window thread while that window and its host are retained.
    /// # Errors
    /// Preserves capture, timer, document and geometry failures.
    pub fn message(
        &self,
        host: &impl ScrollHost,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<bool, NativeUiError> {
        match message {
            WM_MOUSEWHEEL => return self.wheel.message(host, wparam),
            WM_TIMER if wparam.0 == ANIMATION && host.owns_timer(wparam.0) => {
                self.animate(host)?;
                return Ok(true);
            }
            WM_TIMER if wparam.0 == REPEAT && host.owns_timer(wparam.0) => {
                self.repeat(host)?;
                return Ok(true);
            }
            WM_CAPTURECHANGED => {
                if self.captured.get() {
                    self.cancel(host, false)?;
                }
                return Ok(false);
            }
            WM_CANCELMODE | WM_KILLFOCUS => {
                self.cancel(host, true)?;
                return Ok(false);
            }
            WM_ENABLE | WM_SHOWWINDOW if wparam.0 == 0 => {
                self.cancel(host, true)?;
                return Ok(false);
            }
            WM_MOUSELEAVE => {
                self.tracking.set(false);
                self.hover(host, self.pressed.get() != Part::None)?;
                return Ok(false);
            }
            WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_LBUTTONUP => {}
            _ => return Ok(false),
        }
        let point = POINT { x: lparam.0 as i16 as i32, y: (lparam.0 >> 16) as i16 as i32 };
        self.point.set(point);
        let Some(model) = self.model(host)? else {
            if self.captured.get() {
                self.cancel(host, true)?;
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
                    hwndTrack: host.window(),
                    dwHoverTime: 0,
                };
                // SAFETY: Track leave notifications only for this original native input child.
                unsafe { TrackMouseEvent(&mut tracking) }
                    .map_err(|cause| NativeUiError::new("console_scroll_track", cause))?;
                self.tracking.set(true);
            }
            self.hover(host, over || self.pressed.get() != Part::None)?;
            if self.pressed.get() == Part::Thumb {
                let position = scrollbar::drag_position(&model, point.y, self.grab.get())?;
                self.position(host, position)?;
            }
            return Ok(over || self.captured.get());
        }
        if message == WM_LBUTTONUP && self.captured.get() {
            self.cancel(host, true)?;
            self.hover(host, over)?;
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
                SetCapture(host.window());
            }
            // SAFETY: Observe capture on this same UI thread before accepting the gesture.
            if unsafe { GetCapture() } != host.window() {
                return Err(NativeUiError::message(
                    "console_scroll_capture",
                    "native capture identity differs",
                ));
            }
            self.captured.set(true);
            self.pressed.set(part);
            self.hover(host, true)?;
            if part == Part::Thumb {
                self.grab.set(point.y - geometry.thumb.top);
            } else {
                self.step(host, part)?;
                self.repeating.set(false);
                host.set_timer(REPEAT, 500)?;
            }
            host.invalidate();
            return Ok(true);
        }
        Ok(false)
    }
    /// Cancel only this gesture's capture/timers; preserve failed native obligations for retry.
    /// The host and creating window thread must remain alive until this succeeds.
    /// # Errors
    /// Preserves the first failure while attempting every outstanding retirement.
    pub fn cancel(&self, host: &impl ScrollHost, release: bool) -> Result<(), NativeUiError> {
        let mut failure = None;
        for id in [ANIMATION, REPEAT] {
            if let Err(cause) = host.kill_timer(id) {
                NativeUiError::retain_first(
                    &mut failure,
                    NativeUiError::new("console_scroll_timer_retire", cause),
                );
            }
        }
        if self.captured.get() {
            // SAFETY: Capture identity is observed before releasing any native input ownership.
            if release && unsafe { GetCapture() } == host.window() {
                // SAFETY: Only this owner's original successful capture is released.
                if let Err(cause) = unsafe { ReleaseCapture() } {
                    NativeUiError::retain_first(
                        &mut failure,
                        NativeUiError::new("console_scroll_capture_retire", cause),
                    );
                }
            }
            // SAFETY: Keep custody if native capture removal failed; never release another window.
            self.captured.set(unsafe { GetCapture() } == host.window() && release);
        }
        self.pressed.set(Part::None);
        self.repeating.set(false);
        self.from.set(0);
        self.target.set(0);
        self.expanded.set(false);
        host.invalidate();
        failure.map_or(Ok(()), Err)
    }
    fn hover(&self, host: &impl ScrollHost, expanded: bool) -> Result<(), NativeUiError> {
        if self.expanded.replace(expanded) == expanded {
            return Ok(());
        }
        let now = host.now_millis();
        self.from.set(progress(
            self.from.get(),
            self.target.get(),
            now.saturating_sub(self.started.get()),
        ));
        self.target.set(if expanded { 255 } else { 0 });
        self.started.set(now);
        host.set_timer(ANIMATION, 16)?;
        host.invalidate();
        Ok(())
    }
    fn animate(&self, host: &impl ScrollHost) -> Result<(), NativeUiError> {
        if host.now_millis().saturating_sub(self.started.get()) >= DURATION {
            host.kill_timer(ANIMATION)
                .map_err(|cause| NativeUiError::new("console_scroll_animation_retire", cause))?;
            self.from.set(self.target.get());
        }
        host.invalidate();
        Ok(())
    }
    fn repeat(&self, host: &impl ScrollHost) -> Result<(), NativeUiError> {
        if self.pressed.get() == Part::None {
            return self.cancel(host, true);
        }
        if !self.repeating.replace(true) {
            host.set_timer(REPEAT, 50)?;
        }
        if let Some(model) = self.model(host)?
            && scrollbar::hit_test(&model, self.point.get())? == self.pressed.get()
        {
            self.step(host, self.pressed.get())?;
        }
        Ok(())
    }
    fn step(&self, host: &impl ScrollHost, part: Part) -> Result<(), NativeUiError> {
        let command = match part {
            Part::Decrement => ScrollStep::LineBefore,
            Part::Increment => ScrollStep::LineAfter,
            Part::PageBefore => ScrollStep::PageBefore,
            Part::PageAfter => ScrollStep::PageAfter,
            _ => return Ok(()),
        };
        host.scroll(command)?;
        host.invalidate();
        Ok(())
    }
    fn position(&self, host: &impl ScrollHost, position: i32) -> Result<(), NativeUiError> {
        host.set_position(position)?;
        host.invalidate();
        Ok(())
    }
}
/// Qt's numeric hover animation is linear, exact at both endpoints and reversible mid-flight.
fn progress(from: u8, to: u8, elapsed: u64) -> u8 {
    let elapsed = elapsed.min(DURATION) as i64;
    (i64::from(from) + (i64::from(to) - i64::from(from)) * elapsed / DURATION as i64) as u8
}
#[cfg(test)]
#[path = "scroll_input_tests.rs"]
mod tests;
