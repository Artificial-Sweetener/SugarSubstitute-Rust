// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native accessible button peers while the content owner paints their pixels.
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

#[path = "windows_controls_creation.rs"]
mod creation;
use creation::{Creation, NativePort};
#[path = "windows_controls_retry.rs"]
mod retry;
#[path = "windows_controls_state.rs"]
mod state;
#[path = "windows_controls_visibility.rs"]
mod visibility;

/// Retain a stable native identity and the last applied content snapshot.
struct Peer {
    hwnd: HWND,
    snapshot: AccessibleControl,
    rect: RECT,
    registration: Option<Registration<Callback>>,
}

/// Select authoritative pixels while preserving the same native accessible lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaintMode {
    /// Splash content owns visuals; peers provide accessibility and keyboard focus.
    PreparedPixels,
    /// The OS BUTTON procedure owns main caption visuals and pointer input.
    NativeButtons(crate::MainIdentity),
}

/// Own the native peers and subclass borrows until the surface ends.
pub(crate) struct Controls {
    window: Rc<Window>,
    callback: Rc<Callback>,
    creation: Creation<Registration<Callback>, Peer>,
    planned: Vec<AccessibleControl>,
    paint_mode: PaintMode,
}

impl Controls {
    /// Allocate inert retained callback/control data before any native initialization.
    pub(crate) fn new(
        window: Rc<Window>,
        proxy: EventLoopProxy<NativeAction>,
        controls: &[AccessibleControl],
        paint_mode: PaintMode,
        timings: Option<crate::timing::TimingObserver>,
    ) -> Result<Self, DesktopError> {
        validate_controls(controls)?;
        let callback = Rc::new(Callback::new(proxy, timings, paint_mode, controls.len())?);
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
        Ok(Self { window, callback, creation: Creation::new(controls.len())?, planned, paint_mode })
    }

    /// Include the largest current peers, actions, subclass Rc states and label conversion.
    pub(crate) fn storage_bytes(count: usize) -> Option<usize> {
        size_of::<Self>()
            .checked_add(count.checked_mul(size_of::<AccessibleControl>())?)?
            .checked_add(Creation::<Registration<Callback>, Peer>::storage_bytes(count)?)?
            .checked_add(Callback::storage_bytes(count)?)?
            .checked_add(
                count.checked_add(1)?.checked_mul(Registration::<Callback>::allocation_bytes())?,
            )?
            .checked_add(count.checked_mul(129 * size_of::<u16>())?)
    }

    /// Initialize once, with this owner already retained by its native surface.
    pub(crate) fn initialize(&mut self) -> Result<(), DesktopError> {
        let mut port = ControlPort {
            window: &self.window,
            callback: &self.callback,
            planned: &self.planned,
            paint_mode: self.paint_mode,
        };
        self.creation.initialize(&mut port)
    }

    /// Apply labels, DPI bounds and availability without reconstructing native identities.
    pub(crate) fn update(&mut self, controls: &[AccessibleControl]) -> Result<(), DesktopError> {
        if !self.creation.writable() {
            return Err(DesktopError::message(
                "update_accessible_controls",
                "control initialization is incomplete or closing",
            ));
        }
        validate_controls(controls)?;
        state::validate_inventory(&self.planned, controls)?;
        for (control, peer) in
            controls.iter().zip(self.creation.peers.iter_mut().map(|slot| &mut slot.value))
        {
            if !control.visible {
                visibility::update(
                    &self.callback,
                    self.creation.parent.as_ref(),
                    peer,
                    self.paint_mode,
                    *control,
                )?;
            }
            if control.label != peer.snapshot.label {
                let label: Vec<u16> = control.label.encode_utf16().chain(Some(0)).collect();
                // SAFETY: The live child copies the temporary UTF-16 label synchronously.
                unsafe { SetWindowTextW(peer.hwnd, PCWSTR(label.as_ptr())) }
                    .map_err(|error| DesktopError::new("update_accessible_label", error))?;
            }
            let rect = physical_bounds(control, self.window.scale_factor())?;
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
            if control.visible {
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
        Ok(())
    }

    /// End every native callback borrow before returning a surface outcome.
    /// Any OS cleanup failure remains an observable failed terminal result.
    pub(crate) fn shutdown(&mut self) -> Result<(), DesktopError> {
        let mut port = ControlPort {
            window: &self.window,
            callback: &self.callback,
            planned: &self.planned,
            paint_mode: self.paint_mode,
        };
        self.creation.shutdown(&mut port)
    }

    /// Draw a native focus cue once after each full CPU blit; never XOR an existing cue twice.
    pub(crate) fn draw_focus(&self) -> Result<(), DesktopError> {
        if !self.callback.prepared_pixels() {
            return Ok(());
        }
        // SAFETY: GetFocus only reads this calling UI thread's focus identity.
        let focus = unsafe { GetFocus() };
        if let Some(peer) = self
            .creation
            .peers
            .iter()
            .map(|slot| &slot.value)
            .find(|peer| peer.snapshot.visible && peer.hwnd == focus)
        {
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
        self.creation.retired(Rc::strong_count(&self.callback) == 1)
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

/// Borrow the exact retained parent/callback and bounded planned metadata for native effects.
struct ControlPort<'a> {
    window: &'a Window,
    callback: &'a Rc<Callback>,
    planned: &'a [AccessibleControl],
    paint_mode: PaintMode,
}
impl NativePort for ControlPort<'_> {
    type Parent = Registration<Callback>;
    type Peer = Peer;
    fn parent(&mut self) -> Result<Self::Parent, DesktopError> {
        Ok(Registration::new(self.callback))
    }
    fn install_parent(&mut self, parent: &Self::Parent) -> Result<(), DesktopError> {
        // SAFETY: This thread owns the retained parent and exact Callback registration.
        unsafe { install(HWND(self.window.hwnd() as *mut c_void), Some(parent_proc), parent) }
    }
    fn peer(&mut self, index: usize) -> Result<Self::Peer, DesktopError> {
        let control = *self.planned.get(index).ok_or_else(|| {
            DesktopError::message("create_accessible_control", "planned identity missing")
        })?;
        let label: Vec<u16> = control.label.encode_utf16().chain(Some(0)).collect();
        let rect = physical_bounds(&control, self.window.scale_factor())?;
        // SAFETY: The owned parent belongs to this thread; BUTTON copies the bounded label.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(label.as_ptr()),
                retry::initial_style(self.paint_mode, control.id, control.visible),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                Some(HWND(self.window.hwnd() as *mut c_void)),
                Some(HMENU(usize::from(control.id) as *mut c_void)),
                None,
                None,
            )
        }
        .map_err(|cause| DesktopError::new("create_accessible_control", cause))?;
        Ok(Peer {
            hwnd,
            snapshot: control,
            rect,
            registration: Some(Registration::new(self.callback)),
        })
    }
    fn register_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError> {
        self.callback.register(peer.hwnd, peer.snapshot.id, action(self.paint_mode, peer.snapshot))
    }
    fn install_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError> {
        let registration = peer.registration.as_ref().ok_or_else(|| {
            DesktopError::message("register_accessible_child", "retained registration missing")
        })?;
        // SAFETY: The transaction retains this exact child and Callback before native reentry.
        unsafe { install(peer.hwnd, Some(child_proc), registration) }
    }
    fn destroyed(&self, peer: &Self::Peer, parent: Option<&Self::Parent>) -> bool {
        peer.registration.as_ref().is_some_and(Registration::destroyed)
            || parent.is_some_and(Registration::destroyed)
    }
    fn remove_peer(&mut self, peer: &mut Self::Peer) -> Result<(), DesktopError> {
        if let Some(registration) = &peer.registration {
            if registration.installed() {
                remove(peer.hwnd, Some(child_proc), registration)?;
            }
            peer.registration = None;
        }
        Ok(())
    }
    fn destroy_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError> {
        // SAFETY: The transaction observed successful callback removal before this effect.
        unsafe { DestroyWindow(peer.hwnd) }
            .map_err(|cause| DesktopError::new("destroy_accessible_control", cause))
    }
    fn retire_parent(&mut self, parent: &Self::Parent) -> Result<(), DesktopError> {
        if parent.destroyed() || !parent.installed() {
            return Ok(());
        }
        remove(HWND(self.window.hwnd() as *mut c_void), Some(parent_proc), parent)
    }
    fn callbacks_retired(&self) -> bool {
        Rc::strong_count(self.callback) == 1
    }
}

/// Derive callback eligibility from the same current snapshot before native input can route.
fn action(mode: PaintMode, control: AccessibleControl) -> Option<NativeAction> {
    if !control.visible {
        return None;
    }
    match mode {
        PaintMode::NativeButtons(_) if control.id == 4 => None,
        PaintMode::NativeButtons(identity) => Some(NativeAction::MainControl(identity, control.id)),
        PaintMode::PreparedPixels if control.close => Some(NativeAction::Close),
        PaintMode::PreparedPixels => Some(NativeAction::Activate(control.id)),
    }
}
