// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt a prepared page viewport to the console's shared Fluent scrollbar and native gestures.
use crate::{
    DesktopError,
    windows_subclass::{Registration, install, remove},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use sugarsubstitute_contracts::presentation::PreparedFrame;
use sugarsubstitute_ui::{
    widgets::Bounds,
    windows::{
        scroll_input::ScrollInput,
        scrollbar::{Model, Part},
    },
};
use tao::{platform::windows::WindowExtWindows, window::Window};
use windows::Win32::Foundation::{HWND, RECT};
mod host;
mod paint;
mod procedure;

const UNITS: f32 = 64.0;

/// One original native parent retains callbacks, animation timers and admitted overlay pixels.
pub(crate) struct ScrollViewport {
    window: Rc<Window>,
    state: Rc<State>,
    registration: Registration<State>,
}
struct State {
    hwnd: HWND,
    input: ScrollInput,
    model: Cell<Option<Model>>,
    pending: Cell<Option<i32>>,
    timers: Cell<[usize; 2]>,
    active: Cell<bool>,
    dirty: Cell<bool>,
    failure: RefCell<Option<DesktopError>>,
    paint: RefCell<Option<paint::Paint>>,
}
impl ScrollViewport {
    #[cfg(any(test, feature = "boundary-fixtures"))]
    /// Observe the existing native overlay after its normal paint has settled.
    pub(crate) fn capture(
        &self,
        observer: &mut dyn crate::pixel_capture::PixelCapture,
    ) -> Result<(), DesktopError> {
        if let Some(paint) = self.state.paint.borrow_mut().as_mut() {
            paint.capture(observer)?;
        }
        Ok(())
    }

    /// Construct inert custody before installing a native callback. Call `shutdown` before release.
    pub(crate) fn new(window: Rc<Window>) -> Self {
        let state = Rc::new(State::new(HWND(window.hwnd() as *mut std::ffi::c_void)));
        Self { window, registration: Registration::new(&state), state }
    }
    /// Register only after the containing window lifetime has retained this owner.
    pub(crate) fn initialize(&self) -> Result<(), DesktopError> {
        // SAFETY: This parent and the exact retained registration belong to the creating UI thread.
        unsafe { install(self.state.hwnd, Some(procedure::procedure), &self.registration) }
    }
    /// Accept authoritative prepared geometry; pointer state survives ordinary repaint.
    pub(crate) fn present(
        &self,
        frame: &PreparedFrame,
        bounds: Bounds,
        content: f32,
        position: f32,
    ) -> Result<(), DesktopError> {
        self.check()?;
        if !self.state.active.get() || !self.registration.installed() {
            return Err(DesktopError::message(
                "native_scroll_present",
                "Original scroll owner is retired",
            ));
        }
        let model = geometry(
            frame,
            bounds,
            content,
            position,
            self.window.theme() == tao::window::Theme::Dark,
        )?;
        let changed = self.state.model.get().is_some_and(|old| {
            model.is_none_or(|new| old.bounds != new.bounds || old.scale != new.scale)
        });
        if changed {
            self.state.input.cancel(self.state.as_ref(), true)?;
        }
        if let Some(model) = model {
            let mut paint = self.state.paint.borrow_mut();
            if let Some(owner) = paint.as_mut() {
                owner.prepare(frame, &model)?;
            } else {
                *paint = Some(paint::Paint::new(frame, &model)?);
            }
        } else {
            self.state.input.cancel(self.state.as_ref(), true)?;
            if let Some(owner) = self.state.paint.borrow_mut().as_mut() {
                owner.close()?;
            }
            self.state.paint.borrow_mut().take();
        }
        self.state.model.set(model);
        self.state.dirty.set(true);
        Ok(())
    }
    /// Coalesce native gestures without calling application navigation inside a window procedure.
    pub(crate) fn take_position(&self) -> Result<Option<f32>, DesktopError> {
        self.check()?;
        Ok(self.state.pending.take().map(|position| position as f32 / UNITS))
    }
    /// Apply a viewport key from an original prepared peer without changing native text selection.
    pub(crate) fn key(
        &self,
        key: sugarsubstitute_ui::windows::scroll_port::ScrollKey,
    ) -> Result<(), DesktopError> {
        self.check()?;
        self.state.input.key(self.state.as_ref(), key)?;
        Ok(())
    }
    /// Observe only the shared widget's actual pointer-capture ownership.
    pub(crate) fn captured(&self) -> bool {
        self.state.input.owns_capture()
    }
    /// Reuse the original native alpha buffer after the prepared parent has been exposed.
    pub(crate) fn paint(&self) -> Result<(), DesktopError> {
        self.check()?;
        self.state.paint(false)
    }
    /// Release capture and timers before removing the exact callback and its retained frame.
    pub(crate) fn shutdown(&self) -> Result<(), DesktopError> {
        self.state.active.set(false);
        if self.registration.destroyed() {
            self.state.timers.set([0; 2]);
        }
        self.state.input.cancel(self.state.as_ref(), !self.registration.destroyed())?;
        if let Some(paint) = self.state.paint.borrow_mut().as_mut() {
            paint.close()?;
        }
        self.state.paint.borrow_mut().take();
        if self.registration.installed() {
            remove(self.state.hwnd, Some(procedure::procedure), &self.registration)?;
        }
        self.state.model.set(None);
        self.state.pending.set(None);
        self.check()
    }
    /// Surface native callback failures at the event-loop boundary with their original cause.
    fn check(&self) -> Result<(), DesktopError> {
        match self.state.failure.borrow_mut().take() {
            Some(cause) => Err(cause),
            None => Ok(()),
        }
    }
}
impl Drop for ScrollViewport {
    fn drop(&mut self) {
        if self.registration.installed()
            || self.state.timers.get() != [0; 2]
            || self.state.paint.borrow().is_some()
        {
            // Preserve affine ownership if a checked native retirement failed.
            std::mem::forget(self.window.clone());
            std::mem::forget(self.state.clone());
            tracing::error!(
                operation = "native_scroll_unresolved_drop",
                "Retaining unresolved native scrollbar custody"
            );
        }
    }
}
impl State {
    /// Keep native input custody inert until the exact parent registration is installed.
    fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            input: ScrollInput::new(),
            model: Cell::new(None),
            pending: Cell::new(None),
            timers: Cell::new([0; 2]),
            active: Cell::new(true),
            dirty: Cell::new(true),
            failure: RefCell::new(None),
            paint: RefCell::new(None),
        }
    }

    /// Keep the first native failure until the owning event loop observes it.
    fn record(&self, cause: DesktopError) {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() {
            *failure = Some(cause);
        }
    }
    /// Paint only the shared scrollbar over the accepted page's gutter.
    fn paint(&self, force: bool) -> Result<(), DesktopError> {
        if !self.active.get() || (!force && !self.dirty.get()) {
            return Ok(());
        }
        let Some(model) = self.input.model(self)? else {
            return Ok(());
        };
        let Ok(mut retained) = self.paint.try_borrow_mut() else {
            self.dirty.set(true);
            return Ok(());
        };
        if let Some(paint) = retained.as_mut() {
            paint.draw(self.hwnd, model, self.dirty.replace(false))?;
        }
        Ok(())
    }
}
/// Convert a validated logical page into the shared scrollbar's fixed-point document coordinates.
fn geometry(
    frame: &PreparedFrame,
    b: Bounds,
    content: f32,
    position: f32,
    dark: bool,
) -> Result<Option<Model>, DesktopError> {
    b.validate(frame.scale() as f32).map_err(|e| DesktopError::new("native_scroll_bounds", e))?;
    let maximum = (content - b.height).max(0.0);
    if !content.is_finite()
        || !position.is_finite()
        || content < 0.0
        || content * UNITS > i32::MAX as f32
        || position < 0.0
        || position > maximum
    {
        return Err(DesktopError::message("native_scroll_range", "Invalid prepared scroll range"));
    }
    if maximum == 0.0 {
        return Ok(None);
    }
    let scale = frame.scale();
    let rect = RECT {
        left: (f64::from(b.x) * scale).round() as i32,
        top: (f64::from(b.y) * scale).round() as i32,
        right: (f64::from(b.x + b.width) * scale).round() as i32,
        bottom: (f64::from(b.y + b.height) * scale).round() as i32,
    };
    crate::windows_alpha::viewport(frame, rect)?;
    Ok(Some(Model {
        appearance: sugarsubstitute_ui::windows::scrollbar::Appearance::Gutter,
        bounds: rect,
        scale,
        minimum: 0,
        maximum: (maximum * UNITS).round() as i32,
        page: (b.height * UNITS).round() as u32,
        position: (position * UNITS).round() as i32,
        dark,
        enabled: true,
        hover: 0,
        pressed: Part::None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Native drawing/allocation may pump a second paint; the original borrow must remain intact.
    #[test]
    fn reentrant_page_paint_defers_without_borrowing_native_storage_twice()
    -> Result<(), DesktopError> {
        let state = State::new(HWND::default());
        state.model.set(Some(Model {
            appearance: sugarsubstitute_ui::windows::scrollbar::Appearance::Gutter,
            bounds: RECT { left: 0, top: 0, right: 15, bottom: 200 },
            scale: 1.0,
            minimum: 0,
            maximum: 100,
            page: 100,
            position: 0,
            dark: true,
            enabled: true,
            hover: 0,
            pressed: Part::None,
        }));
        state.dirty.set(false);
        let retained = state.paint.borrow_mut();
        state.paint(true)?;
        assert!(state.dirty.get());
        assert!(retained.is_none());
        drop(retained);
        state.active.set(false);
        state.paint(true)?;
        Ok(())
    }
}
