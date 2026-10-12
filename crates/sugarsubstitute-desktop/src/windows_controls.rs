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
                SetWindowPos, SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_TABSTOP,
                WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, w},
};

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
    peers: Vec<Peer>,
    parent_registration: Option<Registration<Callback>>,
    teardown_error: Option<DesktopError>,
}

impl Controls {
    /// Register standard BUTTON peers whose default OS providers expose name, role and invoke.
    pub(crate) fn new(
        window: Rc<Window>,
        proxy: EventLoopProxy<NativeAction>,
        controls: &[AccessibleControl],
        paint_mode: PaintMode,
        timings: Option<crate::timing::TimingObserver>,
    ) -> Result<Self, DesktopError> {
        validate_controls(controls)?;
        let parent = HWND(window.hwnd() as *mut c_void);
        let callback = Rc::new(Callback::new(proxy, timings, paint_mode, controls.len())?);
        let mut owner = Self {
            window,
            callback,
            peers: Vec::new(),
            parent_registration: None,
            teardown_error: None,
        };
        owner
            .peers
            .try_reserve_exact(controls.len())
            .map_err(|cause| DesktopError::new("allocate_accessible_peers", cause))?;
        if owner.peers.capacity() != controls.len() {
            return Err(DesktopError::message(
                "allocate_accessible_peers",
                "peer capacity exceeded",
            ));
        }
        owner.parent_registration = Some(Registration::new(&owner.callback));
        // SAFETY: This thread owns parent and parent_proc consumes the transferred Rc<Callback>.
        unsafe {
            install(
                parent,
                Some(parent_proc),
                owner.parent_registration.as_ref().ok_or_else(|| {
                    DesktopError::message(
                        "register_accessible_parent",
                        "missing parent registration",
                    )
                })?,
            )?;
        }
        for control in controls {
            let label: Vec<u16> = control.label.encode_utf16().chain(Some(0)).collect();
            let rect = physical_bounds(control, owner.window.scale_factor())?;
            // SAFETY: BUTTON is an OS-owned class; the label is copied synchronously.
            // The nonzero control ID is a child menu identity, not an owned menu handle.
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("BUTTON"),
                    PCWSTR(label.as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    Some(parent),
                    Some(HMENU(usize::from(control.id) as *mut c_void)),
                    None,
                    None,
                )
            }
            .map_err(|error| DesktopError::new("create_accessible_control", error))?;
            owner.peers.push(Peer {
                hwnd,
                snapshot: *control,
                rect,
                registration: Some(Registration::new(&owner.callback)),
            });
            owner.callback.register(
                hwnd,
                control.id,
                match paint_mode {
                    PaintMode::NativeButtons(identity) => {
                        NativeAction::MainControl(identity, control.id)
                    }
                    PaintMode::PreparedPixels if control.close => NativeAction::Close,
                    PaintMode::PreparedPixels => NativeAction::Activate(control.id),
                },
            )?;
            // SAFETY: This thread owns child and child_proc consumes the transferred Rc<Callback>.
            unsafe {
                let registration = owner
                    .peers
                    .last()
                    .and_then(|peer| peer.registration.as_ref())
                    .ok_or_else(|| {
                    DesktopError::message("register_accessible_child", "missing child registration")
                })?;
                install(hwnd, Some(child_proc), registration)?;
            }
        }
        Ok(owner)
    }

    /// Apply disclosure labels and DPI bounds without reconstructing native identities.
    pub(crate) fn update(&mut self, controls: &[AccessibleControl]) -> Result<(), DesktopError> {
        validate_controls(controls)?;
        if controls.len() != self.peers.len() {
            return Err(DesktopError::message(
                "update_accessible_controls",
                "control inventory changed during the surface lifetime",
            ));
        }
        for (control, peer) in controls.iter().zip(&mut self.peers) {
            if control.id != peer.snapshot.id || control.close != peer.snapshot.close {
                return Err(DesktopError::message(
                    "update_accessible_controls",
                    "control identity or native action changed",
                ));
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
            peer.snapshot = *control;
        }
        Ok(())
    }

    /// End every native callback borrow before returning a surface outcome.
    /// Any OS cleanup failure remains an observable failed terminal result.
    pub(crate) fn shutdown(&mut self) -> Result<(), DesktopError> {
        // SAFETY: Native peers and subclass borrows belong to this creating thread.
        // Removal precedes destruction and the callback allocation remains live here.
        unsafe {
            let mut remaining = Vec::new();
            remaining
                .try_reserve_exact(self.peers.len())
                .map_err(|cause| DesktopError::new("allocate_control_retirement", cause))?;
            for mut peer in self.peers.drain(..) {
                if peer.registration.as_ref().is_some_and(Registration::destroyed)
                    || self.parent_registration.as_ref().is_some_and(Registration::destroyed)
                {
                    continue;
                }
                if let Some(registration) = &peer.registration {
                    if registration.installed()
                        && let Err(cause) = remove(peer.hwnd, Some(child_proc), registration)
                    {
                        DesktopError::retain_first(&mut self.teardown_error, cause);
                        remaining.push(peer);
                        continue;
                    }
                    peer.registration = None;
                }
                if let Err(error) = DestroyWindow(peer.hwnd) {
                    DesktopError::retain_first(
                        &mut self.teardown_error,
                        DesktopError::new("destroy_accessible_control", error),
                    );
                    remaining.push(peer);
                }
            }
            self.peers = remaining;
            if self.peers.is_empty()
                && let Some(registration) = &self.parent_registration
            {
                let parent = HWND(self.window.hwnd() as *mut c_void);
                if registration.destroyed() || !registration.installed() {
                    self.parent_registration = None;
                } else {
                    match remove(parent, Some(parent_proc), registration) {
                        Ok(()) => self.parent_registration = None,
                        Err(cause) => DesktopError::retain_first(&mut self.teardown_error, cause),
                    }
                }
            }
        }
        if self.peers.is_empty()
            && self.parent_registration.is_none()
            && Rc::strong_count(&self.callback) != 1
        {
            self.teardown_error.get_or_insert_with(|| {
                DesktopError::message(
                    "release_accessible_callbacks",
                    "native callback references survived completed subclass teardown",
                )
            });
        }
        match self.teardown_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Draw a native focus cue once after each full CPU blit; never XOR an existing cue twice.
    pub(crate) fn draw_focus(&self) -> Result<(), DesktopError> {
        if !self.callback.prepared_pixels() {
            return Ok(());
        }
        // SAFETY: GetFocus only reads this calling UI thread's focus identity.
        let focus = unsafe { GetFocus() };
        if let Some(peer) = self.peers.iter().find(|peer| peer.hwnd == focus) {
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
        self.peers.is_empty() && self.parent_registration.is_none()
    }
}

impl Drop for Controls {
    /// Never invoke native cleanup during unwinding; retain unresolved parents before diagnostics.
    fn drop(&mut self) {
        if !self.retired() {
            std::mem::forget(self.window.clone());
            std::mem::forget(self.callback.clone());
            std::mem::forget(std::mem::take(&mut self.peers));
            std::mem::forget(self.parent_registration.take());
            tracing::error!(
                operation = "native_controls_unresolved_drop",
                "Retaining native controls until process termination"
            );
        }
    }
}
