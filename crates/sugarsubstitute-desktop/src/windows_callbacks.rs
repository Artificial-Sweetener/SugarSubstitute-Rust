// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exactly one callback-data reference per installed native subclass.
//!
//! Subclasses are installed/removed only on their creating UI thread. Each
//! successful install transfers one Rc clone to Win32; explicit successful removal
//! or WM_NCDESTROY returns that clone exactly once. Unrelated procedures remain chained.

use crate::{DesktopError, runtime::NativeAction, windows_subclass::destroy_callback};
use std::cell::RefCell;
#[path = "timing_native_callback.rs"]
mod timing;
use crate::timing::{NativeCallbackKind, TimingObserver};
use tao::event_loop::EventLoopProxy;
use timing::NativeCallbackObservation;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::ValidateRect,
    UI::{
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            HTTRANSPARENT, SC_MOVE, WM_COMMAND, WM_ENTERSIZEMOVE, WM_ERASEBKGND, WM_EXITSIZEMOVE,
            WM_KEYDOWN, WM_KILLFOCUS, WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SETFOCUS,
            WM_SYSCOMMAND,
        },
    },
};

/// Keep immutable peer identities and the UI-only wake proxy behind stable reference ownership.
pub(crate) struct Callback {
    proxy: EventLoopProxy<NativeAction>,
    timings: Option<TimingObserver>,
    controls: RefCell<Vec<(HWND, u16, NativeAction)>>,
}

impl Callback {
    /// Start an empty native inventory before constructing child HWNDs.
    pub(crate) fn new(
        proxy: EventLoopProxy<NativeAction>,
        timings: Option<TimingObserver>,
    ) -> Self {
        Self { proxy, timings, controls: RefCell::new(Vec::new()) }
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
    // SAFETY: Each installed subclass owns its transferred reference through this invocation.
    let (timings, proxy) = {
        // SAFETY: Installed data is live; this short borrow ends before final destruction or forwarding.
        let callback = unsafe { &*(data as *const Callback) };
        (callback.timings.clone(), callback.proxy.clone())
    };
    let mut observation =
        NativeCallbackObservation::new(timings, proxy, NativeCallbackKind::WindowsParent);
    let result = (|| {
        if message == WM_NCDESTROY {
            // SAFETY: This callback still owns its transferred Rc until this final message.
            return unsafe {
                destroy_callback::<Callback>(hwnd, Some(parent_proc), wparam, lparam, data)
            };
        }
        // SAFETY: The nonfinal invocation still owns its installed reference.
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
        if let Some(phase) = move_trace_phase(message, wparam) {
            tracing::info!(
                operation = "native_caption_move_observed",
                phase,
                "Observed owned parent native movement message"
            );
        }
        // SAFETY: Preserve Tao's procedure and every unrelated native subclass.
        if message == WM_SYSCOMMAND && (wparam.0 & 0xfff0) == SC_MOVE as usize {
            observation.delegated(|| {
                // SAFETY: Forward this live owned HWND and unchanged message through its subclass chain.
                unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
            })
        } else {
            // SAFETY: Forward this live owned HWND and unchanged message through its subclass chain.
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
    })();
    observation.finish();
    result
}

/// Identify native movement diagnostics without retaining data across default-procedure reentry.
/// The owned surface is nonresizable; move-loop entry/exit refer to that same parent window.
fn move_trace_phase(message: u32, wparam: WPARAM) -> Option<&'static str> {
    match message {
        WM_SYSCOMMAND if (wparam.0 & 0xfff0) == SC_MOVE as usize => Some("command"),
        WM_ENTERSIZEMOVE => Some("entered"),
        WM_EXITSIZEMOVE => Some("exited"),
        _ => None,
    }
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
    // SAFETY: Each installed subclass owns its transferred reference through this invocation.
    let (timings, proxy) = {
        // SAFETY: Installed data is live; this short borrow ends before final destruction or forwarding.
        let callback = unsafe { &*(data as *const Callback) };
        (callback.timings.clone(), callback.proxy.clone())
    };
    let observation =
        NativeCallbackObservation::new(timings, proxy, NativeCallbackKind::WindowsChild);
    let result = (|| {
        if message == WM_NCDESTROY {
            // SAFETY: This callback still owns its transferred Rc until this final message.
            return unsafe {
                destroy_callback::<Callback>(hwnd, Some(child_proc), wparam, lparam, data)
            };
        }
        // SAFETY: The nonfinal invocation still owns its installed reference.
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
    })();
    observation.finish();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, SC_CLOSE, SC_SIZE};

    /// Trace both caption and keyboard move commands while leaving unrelated traffic silent.
    #[test]
    fn native_move_trace_filters_commands_without_changing_native_parameters() {
        assert_eq!(move_trace_phase(WM_SYSCOMMAND, WPARAM(SC_MOVE as usize)), Some("command"));
        assert_eq!(
            move_trace_phase(WM_SYSCOMMAND, WPARAM((SC_MOVE | HTCAPTION) as usize)),
            Some("command")
        );
        assert_eq!(move_trace_phase(WM_ENTERSIZEMOVE, WPARAM(0)), Some("entered"));
        assert_eq!(move_trace_phase(WM_EXITSIZEMOVE, WPARAM(0)), Some("exited"));
        for command in [SC_CLOSE, SC_SIZE] {
            assert_eq!(move_trace_phase(WM_SYSCOMMAND, WPARAM(command as usize)), None);
        }
        assert_eq!(move_trace_phase(WM_COMMAND, WPARAM(SC_MOVE as usize)), None);
        assert_eq!(move_trace_phase(WM_PAINT, WPARAM(0)), None);
    }
}
