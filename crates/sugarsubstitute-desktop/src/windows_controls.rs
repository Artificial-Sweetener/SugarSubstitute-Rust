// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native form peers and the bounded console through one checked retirement transaction.
//!
//! All HWNDs, callbacks and stable callback data live on the creating event-loop
//! thread. Every installed subclass retains an Rc reference until explicit removal
//! or WM_NCDESTROY; failed OS removal cannot leave a stale callback pointer.
//! Tao's procedure stays intact and the owned parent bounds all child lifetimes.

use crate::{
    AccessibleControl, DesktopError,
    control_geometry::{physical_bounds, validate_controls},
    native_action::NativeAction,
    windows_callbacks::{Callback, child_proc, parent_proc},
    windows_subclass::{Registration, install, remove},
};
use std::{ffi::c_void, rc::Rc};
use tao::{event_loop::EventLoopProxy, platform::windows::WindowExtWindows, window::Window};
use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{DrawFocusRect, GetDC, ReleaseDC},
        UI::{
            Input::KeyboardAndMouse::GetFocus,
            WindowsAndMessaging::{
                BS_PUSHBUTTON, CreateWindowExW, DestroyWindow, HMENU, SWP_NOACTIVATE, SWP_NOZORDER,
                SetWindowPos, SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_DISABLED,
                WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, w},
};

#[path = "windows_controls_caption.rs"]
mod caption;
#[path = "windows_controls_creation.rs"]
mod creation;
#[path = "windows_controls_native.rs"]
mod native;
use native::ControlPort;
#[path = "windows_controls_specification.rs"]
mod specification;
pub(crate) use specification::Specification;
#[path = "windows_controls_accessibility.rs"]
mod accessibility;
#[cfg(any(test, feature = "boundary-fixtures"))]
#[path = "windows_controls_capture.rs"]
mod capture;
#[path = "windows_controls_clip.rs"]
mod clip;
#[cfg(test)]
#[path = "windows_controls_form_tests.rs"]
mod form_tests;
#[path = "windows_controls_values.rs"]
mod values;
use creation::{Creation, NativePort};
#[path = "windows_console.rs"]
pub(crate) mod console;
#[path = "windows_controls_focus.rs"]
mod focus;
#[path = "windows_controls_retry.rs"]
mod retry;
#[path = "windows_controls_stacking.rs"]
mod stacking;
#[path = "windows_controls_state.rs"]
mod state;
#[path = "windows_controls_visibility.rs"]
mod visibility;
pub(crate) use focus::{focus_window, shift_pressed};

/// Retain a stable native identity and the last applied content snapshot.
struct Peer {
    hwnd: HWND,
    snapshot: AccessibleControl,
    rect: RECT,
    registration: Option<Registration<Callback>>,
    role: sugarsubstitute_ui::form::Role,
    named: std::cell::Cell<bool>,
    engine: Option<Rc<crate::windows_text::Engine>>,
}

/// Select authoritative pixels while preserving the same native accessible lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaintMode {
    /// Splash content owns visuals; peers provide accessibility and keyboard focus.
    PreparedPixels,
    /// Native buttons own input/accessibility; the caption owner paints their iconography.
    NativeButtons(crate::MainIdentity),
}

/// Own the native peers and subclass borrows until the surface ends.
pub(crate) struct Controls {
    window: Rc<Window>,
    callback: Rc<Callback>,
    creation: Creation<Registration<Callback>, Peer>,
    planned: Vec<Specification>,
    paint_mode: PaintMode,
    console: Option<console::Console>,
    text_library: crate::windows_text::library::Library,
    fonts: sugarsubstitute_ui::windows::font::Fonts,
    pending_clip: Option<windows::Win32::Graphics::Gdi::HRGN>,
}

impl Controls {
    /// Observe native callback failures without blocking or retiring resources inside a window procedure.
    pub(crate) fn check(&self) -> Result<(), DesktopError> {
        self.callback.check()
    }

    /// Set the native caption palette before its original peers are constructed.
    pub(crate) fn configure_caption(&self, dark: bool, material: crate::NativeMaterial) {
        self.callback.caption.set(Some(crate::windows_callbacks::caption::Caption {
            dark,
            material,
            update: Default::default(),
        }));
    }
    /// Allocate inert retained callback/control data before any native initialization.
    pub(crate) fn new(
        window: Rc<Window>,
        proxy: EventLoopProxy<NativeAction>,
        controls: &[Specification],
        paint_mode: PaintMode,
        timings: Option<crate::timing::TimingObserver>,
    ) -> Result<Self, DesktopError> {
        Self::create(window, proxy, controls, paint_mode, timings, None)
    }

    /// Prepared-pixel peers of a form window whose actions arrive as
    /// [`NativeAction::Form`] under `scope`, apart from the loop's other surfaces.
    pub(crate) fn new_form(
        window: Rc<Window>,
        proxy: EventLoopProxy<NativeAction>,
        controls: &[Specification],
        scope: crate::native_action::FormScope,
    ) -> Result<Self, DesktopError> {
        Self::create(window, proxy, controls, PaintMode::PreparedPixels, None, Some(scope))
    }

    fn create(
        window: Rc<Window>,
        proxy: EventLoopProxy<NativeAction>,
        controls: &[Specification],
        paint_mode: PaintMode,
        timings: Option<crate::timing::TimingObserver>,
        scope: Option<crate::native_action::FormScope>,
    ) -> Result<Self, DesktopError> {
        specification::validate(&[], controls)?;
        let theme = if window.theme() == tao::window::Theme::Dark {
            sugarsubstitute_contracts::configuration::Theme::Dark
        } else {
            sugarsubstitute_contracts::configuration::Theme::Light
        };
        let callback =
            Rc::new(Callback::new(proxy, timings, paint_mode, theme, controls.len(), scope)?);
        let mut planned = Vec::new();
        planned
            .try_reserve_exact(controls.len())
            .map_err(|cause| DesktopError::new("allocate_accessible_plan", cause))?;
        if planned.capacity() != controls.len() {
            return Err(DesktopError::message(
                "allocate_accessible_plan",
                "plan capacity exceeded",
            ));
        }
        planned.extend_from_slice(controls);
        Ok(Self {
            window,
            callback,
            creation: Creation::new(controls.len())?,
            planned,
            paint_mode,
            console: None,
            text_library: crate::windows_text::library::Library::default(),
            pending_clip: None,
            fonts: sugarsubstitute_ui::windows::font::Fonts::new(
                sugarsubstitute_ui::windows::font::FontRole::Form,
            ),
        })
    }

    /// Include the largest current peers, actions, subclass Rc states and label conversion.
    pub(crate) fn storage_bytes(count: usize) -> Option<usize> {
        size_of::<Self>()
            .checked_add(count.checked_mul(size_of::<Specification>())?)?
            .checked_add(Creation::<Registration<Callback>, Peer>::storage_bytes(count)?)?
            .checked_add(Callback::storage_bytes(count)?)?
            .checked_add(
                count.checked_add(1)?.checked_mul(Registration::<Callback>::allocation_bytes())?,
            )?
            .checked_add(count.checked_mul(129 * size_of::<u16>())?)
    }

    /// Admit one shared selector font before native construction or DPI replacement effects.
    fn prepare_select_font(&mut self) -> Result<bool, DesktopError> {
        if !self
            .planned
            .iter()
            .any(|spec| matches!(spec.role, sugarsubstitute_ui::form::Role::Select { .. }))
        {
            return Ok(false);
        }
        // SAFETY: The original retained parent belongs to this native UI thread.
        let dpi = unsafe {
            windows::Win32::UI::HiDpi::GetDpiForWindow(HWND(self.window.hwnd() as *mut c_void))
        };
        Ok(self.fonts.prepare(dpi)?.is_some())
    }
    /// Initialize once, with this owner already retained by its native surface.
    pub(crate) fn initialize(&mut self) -> Result<(), DesktopError> {
        if self
            .planned
            .iter()
            .any(|spec| matches!(spec.role, sugarsubstitute_ui::form::Role::TextField(_)))
            && self.text_library.module().is_none()
        {
            self.text_library.load()?;
        }
        let replacement = self.prepare_select_font()?;
        let mut port = ControlPort {
            window: &self.window,
            callback: &self.callback,
            planned: &self.planned,
            paint_mode: self.paint_mode,
            module: self.text_library.module(),
            font: self.fonts.selected(),
        };
        self.creation.initialize(&mut port)?;
        if replacement {
            self.fonts.commit()?;
        }
        Ok(())
    }

    /// Apply labels, DPI bounds and availability without reconstructing native identities.
    pub(crate) fn update(&mut self, controls: &[Specification]) -> Result<(), DesktopError> {
        if !self.creation.writable() {
            return Err(DesktopError::message(
                "update_accessible_controls",
                "control initialization is incomplete or closing",
            ));
        }
        specification::validate(&self.planned, controls)?;
        let replacement = self.prepare_select_font()?;
        let font = self.fonts.selected();
        for (control, peer) in
            controls.iter().zip(self.creation.peers.iter_mut().map(|slot| &mut slot.value))
        {
            let spec = control;
            let control = &spec.control;
            if !control.availability().actionable() {
                if matches!(spec.role, sugarsubstitute_ui::form::Role::Select { .. }) {
                    sugarsubstitute_ui::windows::select::close(peer.hwnd)?;
                }

                visibility::update(
                    &self.callback,
                    self.creation.parent.as_ref(),
                    peer,
                    self.paint_mode,
                    *control,
                )?;
            }
            values::apply(peer, spec, self.text_library.module(), &self.callback)?;
            if peer.engine.is_none()
                && !matches!(peer.role, sugarsubstitute_ui::form::Role::Select { .. })
                && control.label != peer.snapshot.label
            {
                let label: Vec<u16> = control.label.encode_utf16().chain(Some(0)).collect();
                // SAFETY: The live child copies the temporary UTF-16 label synchronously.
                unsafe { SetWindowTextW(peer.hwnd, PCWSTR(label.as_ptr())) }
                    .map_err(|error| DesktopError::new("update_accessible_label", error))?;
            }
            let rect = values::peer_rect(*control, &spec.role, self.window.scale_factor())?;
            if rect != peer.rect {
                // SAFETY: The retained child is only moved on its creating UI thread.
                unsafe {
                    SetWindowPos(
                        peer.hwnd,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                }
                .map_err(|error| DesktopError::new("update_accessible_bounds", error))?;
                peer.rect = rect;
            }
            if matches!(spec.role, sugarsubstitute_ui::form::Role::Select { .. }) {
                sugarsubstitute_ui::windows::select::configure(
                    peer.hwnd,
                    font.ok_or_else(|| {
                        DesktopError::message("native_select_font", "Font owner absent")
                    })?,
                    rect.bottom - rect.top,
                )?;
            }
            if control.availability().actionable() {
                visibility::update(
                    &self.callback,
                    self.creation.parent.as_ref(),
                    peer,
                    self.paint_mode,
                    *control,
                )?;
            }
            peer.snapshot = *control;
        }
        if replacement {
            self.fonts.commit()?;
        }
        Ok(())
    }

    /// End every native callback borrow before returning a surface outcome.
    /// Any OS cleanup failure remains an observable failed terminal result.
    pub(crate) fn shutdown(&mut self) -> Result<(), DesktopError> {
        self.release_pending_clip()?;
        if let Some(console) = self.console.as_mut() {
            console.shutdown(&self.window)?;
            if !console.retired() {
                return Ok(());
            }
        }
        self.console = None;
        let mut port = ControlPort {
            window: &self.window,
            callback: &self.callback,
            planned: &self.planned,
            paint_mode: self.paint_mode,
            module: self.text_library.module(),
            font: self.fonts.selected(),
        };
        self.creation.shutdown(&mut port)?;
        if self.creation.retired(Rc::strong_count(&self.callback) == 1) {
            self.text_library.close()?;
            self.fonts.retire_after_children()?;
        }
        Ok(())
    }

    /// Draw a native focus cue once after each full CPU blit; never XOR an existing cue twice.
    pub(crate) fn draw_focus(&self) -> Result<(), DesktopError> {
        if !self.callback.prepared_pixels() {
            return Ok(());
        }
        // SAFETY: GetFocus only reads this calling UI thread's focus identity.
        let focus = unsafe { GetFocus() };
        if let Some(peer) = self.creation.peers.iter().map(|slot| &slot.value).find(|peer| {
            peer.engine.is_none() && peer.snapshot.availability().actionable() && peer.hwnd == focus
        }) {
            let mut rect = peer.rect;
            rect.left += 3;
            rect.top += 3;
            rect.right -= 3;
            rect.bottom -= 3;
            let parent = HWND(self.window.hwnd() as *mut c_void);
            // SAFETY: Parent and rectangle remain live. The borrowed DC is released
            // before reporting either drawing or release failure.
            unsafe {
                let dc = GetDC(Some(parent));
                if dc.is_invalid() {
                    return Err(DesktopError::message("draw_native_focus", "GetDC failed"));
                }
                let drawn = DrawFocusRect(dc, &rect);
                let released = ReleaseDC(Some(parent), dc);
                drawn.ok().map_err(|error| DesktopError::new("draw_native_focus", error))?;
                if released == 0 {
                    return Err(DesktopError::message(
                        "release_native_focus_dc",
                        "ReleaseDC failed",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Controls {
    /// Require every peer and native callback registration to be actually retired.
    pub(crate) fn retired(&self) -> bool {
        self.fonts.retired()
            && self.pending_clip.is_none()
            && self.text_library.module().is_none()
            && self.console.as_ref().is_none_or(console::Console::retired)
            && self.creation.retired(Rc::strong_count(&self.callback) == 1)
    }
}

impl Drop for Controls {
    /// Never invoke native cleanup during unwinding; retain unresolved parents before diagnostics.
    fn drop(&mut self) {
        if !self.retired() {
            std::mem::forget(self.window.clone());
            std::mem::forget(self.callback.clone());
            self.creation.retain_unresolved();
            tracing::error!(
                operation = "native_controls_unresolved_drop",
                "Retaining native controls until process termination"
            );
        }
    }
}

/// Derive callback eligibility from the same current snapshot before native input can route.
fn action(mode: PaintMode, control: AccessibleControl) -> Option<NativeAction> {
    if !control.availability().actionable() {
        return None;
    }
    match mode {
        PaintMode::NativeButtons(_) if control.id == 4 => None,
        PaintMode::NativeButtons(identity) => Some(NativeAction::MainControl(identity, control.id)),
        PaintMode::PreparedPixels if control.close => Some(NativeAction::Close),
        PaintMode::PreparedPixels => Some(NativeAction::Activate(control.id)),
    }
}
