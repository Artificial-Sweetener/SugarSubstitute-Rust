// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Forward native focus intent while preserving the standard text/accessibility procedure.
use crate::{
    native_action::NativeAction,
    windows_subclass::{callback, destroy_callback},
};
use std::cell::Cell;
use tao::event_loop::EventLoopProxy;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Controls::EM_SETSEL,
        Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL},
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            SendMessageW, WM_CHAR, WM_CONTEXTMENU, WM_COPY, WM_KEYDOWN, WM_KILLFOCUS, WM_NCDESTROY,
            WM_SETFOCUS,
        },
    },
};
pub(super) struct Callback {
    proxy: EventLoopProxy<NativeAction>,
    timings: Option<crate::timing::TimingObserver>,
    pub(super) active: Cell<bool>,
    pub(super) peer: Cell<Option<HWND>>,
}
impl Callback {
    pub(super) fn new(
        proxy: EventLoopProxy<NativeAction>,
        timings: Option<crate::timing::TimingObserver>,
    ) -> Self {
        Self { proxy, timings, active: Cell::new(false), peer: Cell::new(None) }
    }
    fn send(&self, action: NativeAction) {
        if let Err(cause) = self.proxy.send_event(action) {
            tracing::error!(operation="console_native_delivery",cause=%cause,"Native console event delivery failed");
        }
    }
}
/// # Safety
/// Installed only for the retained Console Callback registration on its creating thread.
pub(super) unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let (timings, proxy) = {
        // SAFETY: The original installed registration remains live until final forwarding.
        let owned = unsafe { callback::<Callback>(data) };
        (owned.timings.clone(), owned.proxy.clone())
    };
    let observation = crate::windows_callbacks::timing::NativeCallbackObservation::new(
        timings,
        proxy,
        crate::timing::NativeCallbackKind::WindowsChild,
    );
    let result = (|| {
        if message == WM_NCDESTROY {
            // SAFETY: The exact installed registration owns this final reference.
            return unsafe {
                destroy_callback::<Callback>(hwnd, Some(procedure), wparam, lparam, data)
            };
        }
        // SAFETY: The installed reference stays live throughout this nonfinal invocation.
        let owned = unsafe { callback::<Callback>(data) };
        if owned.peer.get() != Some(hwnd) {
            // SAFETY: Preserve the native chain without interpreting a foreign identity.
            return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
        }
        if !owned.active.get() && matches!(message, WM_KEYDOWN | WM_CHAR | WM_COPY | WM_CONTEXTMENU)
        {
            return LRESULT(0);
        }
        if message == WM_KEYDOWN {
            match wparam.0 {
                0x09 => {
                    // SAFETY: Read this native UI thread's actual modifier state.
                    owned.send(NativeAction::FocusNext(crate::windows_controls::shift_pressed()));
                    return LRESULT(0);
                }
                0x1b => {
                    owned.send(NativeAction::Close);
                    return LRESULT(0);
                }
                0x41 if control_pressed() => {
                    // SAFETY: Select only the original read-only control's text; no native effect privilege.
                    unsafe {
                        SendMessageW(hwnd, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                    }
                    return LRESULT(0);
                }
                _ => {}
            }
        }
        if message == WM_CHAR && wparam.0 == 9 {
            return LRESULT(0);
        }
        if matches!(message, WM_SETFOCUS | WM_KILLFOCUS) {
            owned.send(NativeAction::FocusChanged);
        }
        // SAFETY: Keep standard mouse/keyboard selection, copy, scrolling and WM_GETOBJECT.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    })();
    observation.finish();
    result
}
fn control_pressed() -> bool {
    // SAFETY: Passive modifier observation on the control's creating UI thread.
    unsafe { GetKeyState(i32::from(VK_CONTROL.0)) < 0 }
}
