// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exactly one callback-data reference per installed native subclass.
//!
//! Subclasses are installed/removed only on their creating UI thread. Each
//! successful install transfers one Rc clone to Win32; explicit successful removal
//! or WM_NCDESTROY returns that clone exactly once. Unrelated procedures remain chained.

use crate::{DesktopError, runtime::NativeAction, windows_subclass::destroy_callback};
use std::cell::RefCell;
use tao::event_loop::EventLoopProxy;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::ValidateRect,
    UI::{
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            HTTRANSPARENT, WM_COMMAND, WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS, WM_NCDESTROY,
            WM_NCHITTEST, WM_PAINT, WM_SETFOCUS,
        },
    },
};

/// Keep immutable peer identities and the UI-only wake proxy behind stable reference ownership.
pub(crate) struct Callback {
    proxy: EventLoopProxy<NativeAction>,
    controls: RefCell<Vec<(HWND, u16, NativeAction)>>,
}

impl Callback {
    /// Start an empty native inventory before constructing child HWNDs.
    pub(crate) fn new(proxy: EventLoopProxy<NativeAction>) -> Self {
        Self { proxy, controls: RefCell::new(Vec::new()) }
    }

    /// Register one created peer; no callback borrow is held across native construction.
    pub(crate) fn register(
        &self,
        hwnd: HWND,
        id: u16,
        action: NativeAction,
    ) -> Result<(), DesktopError> {
        self.controls
            .try_borrow_mut()
            .map_err(|error| DesktopError::new("register_accessible_identity", error))?
            .push((hwnd, id, action));
        Ok(())
    }

    /// Resolve only a registered peer HWND and, for parent notifications, the matching control ID.
    fn action(&self, hwnd: HWND, id: Option<u16>) -> Option<NativeAction> {
        match self.controls.try_borrow() {
            Ok(controls) => controls
                .iter()
                .find(|(peer, registered, _)| {
                    *peer == hwnd && id.is_none_or(|id| id == *registered)
                })
                .map(|(_, _, action)| *action),
            Err(error) => {
                tracing::error!(operation = "resolve_accessible_identity", cause = %error, "Native identity inventory was unexpectedly borrowed");
                None
            }
        }
    }

    /// Wake the native loop without borrowing content or running application work inside a procedure.
    fn send(&self, action: NativeAction) {
        if let Err(error) = self.proxy.send_event(action) {
            tracing::error!(operation = "native_control_delivery", cause = %error, "Native control event could not reach its event loop");
        }
    }
}

/// Route only registered child commands and preserve Tao's native procedure chain.
pub(crate) unsafe extern "system" fn parent_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        // SAFETY: This callback still owns its transferred Rc until this final message.
        return unsafe {
            destroy_callback::<Callback>(hwnd, Some(parent_proc), wparam, lparam, data)
        };
    }
    // SAFETY: This installed subclass owns one raw Rc clone throughout the call.
    let callback = unsafe { &*(data as *const Callback) };
    if message == WM_COMMAND
        && lparam.0 != 0
        && (wparam.0 >> 16) == 0
        && let Some(action) = callback
            .action(HWND(lparam.0 as *mut std::ffi::c_void), Some((wparam.0 & 0xffff) as u16))
    {
        callback.send(action);
        return LRESULT(0);
    }
    // SAFETY: Preserve Tao's procedure and every unrelated native subclass.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Retain OS accessibility/Space activation while the content paints and owns pointer behavior.
pub(crate) unsafe extern "system" fn child_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        // SAFETY: This callback still owns its transferred Rc until this final message.
        return unsafe {
            destroy_callback::<Callback>(hwnd, Some(child_proc), wparam, lparam, data)
        };
    }
    // SAFETY: The installed subclass keeps this allocation live even if its owner was dropped.
    let callback = unsafe { &*(data as *const Callback) };
    match message {
        WM_NCHITTEST => return LRESULT(HTTRANSPARENT as isize),
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            // SAFETY: Validate this child while the content paints its control rectangle.
            if !unsafe { ValidateRect(Some(hwnd), None) }.as_bool() {
                tracing::error!(
                    operation = "validate_accessible_control_paint",
                    "ValidateRect failed"
                );
            }
            return LRESULT(0);
        }
        WM_SETFOCUS | WM_KILLFOCUS => callback.send(NativeAction::FocusChanged),
        WM_KEYDOWN => match wparam.0 {
            0x1b => {
                callback.send(NativeAction::Close);
                return LRESULT(0);
            }
            0x09 => {
                callback.send(NativeAction::Disclosure);
                return LRESULT(0);
            }
            0x0d => {
                if let Some(action) = callback.action(hwnd, None) {
                    callback.send(action);
                }
                return LRESULT(0);
            }
            _ => {}
        },
        _ => {}
    }
    // SAFETY: Preserve WM_GETOBJECT, BM_CLICK and OS-managed focused/pressed button state.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
