// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Forward native focus intent while preserving the standard text/accessibility procedure.
//! Right-click opens the page's text edit rows; the popup's modal loop may retire the console.
use super::menu;
use crate::{
    native_action::NativeAction,
    windows_subclass::{callback, destroy_callback},
};
use std::cell::Cell;
use sugarsubstitute_contracts::localization::edit::EditMessages;
use tao::event_loop::EventLoopProxy;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Controls::EM_SETSEL,
        Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL},
        Shell::{DefSubclassProc, GetWindowSubclass},
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
    names: &'static EditMessages,
    /// The console's theme, which its menu follows.
    pub(super) dark: Cell<bool>,
}
impl Callback {
    pub(super) fn new(
        proxy: EventLoopProxy<NativeAction>,
        timings: Option<crate::timing::TimingObserver>,
        names: &'static EditMessages,
    ) -> Self {
        Self {
            proxy,
            timings,
            active: Cell::new(false),
            peer: Cell::new(None),
            names,
            dark: Cell::new(true),
        }
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
    let mut observation = crate::windows_callbacks::timing::NativeCallbackObservation::new(
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
        if message == WM_CONTEXTMENU {
            let request = menu::Request::new(hwnd, lparam, owned.names, owned.dark.get());
            // The popup's modal loop is not this callback's work, as for a window move.
            let chosen = observation.delegated(|| menu::choose(hwnd, &request));
            // `owned` is not used after the modal loop; the console is checked again.
            let ran = chosen.and_then(|row| match row {
                // SAFETY: This UI thread passes the data this procedure received.
                Some(row) if unsafe { still_open(hwnd, data) } => menu::run(hwnd, row),
                _ => Ok(()),
            });
            if let Err(cause) = ran {
                tracing::error!(operation = "console_menu", cause = %cause);
            }
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
/// Whether the console a menu opened over is still installed and accepting input.
///
/// # Safety
/// Call on the console's UI thread with the `data` its procedure received.
unsafe fn still_open(hwnd: HWND, data: usize) -> bool {
    let mut installed = 0;
    // SAFETY: Query the subclass identity without dereferencing its data.
    let present =
        unsafe { GetWindowSubclass(hwnd, Some(procedure), 1, Some(&raw mut installed)) }.as_bool();
    present && installed == data && {
        // SAFETY: The same registration is still installed, so its data is live.
        let owned = unsafe { callback::<Callback>(data) };
        owned.active.get() && owned.peer.get() == Some(hwnd)
    }
}
fn control_pressed() -> bool {
    // SAFETY: Passive modifier observation on the control's creating UI thread.
    unsafe { GetKeyState(i32::from(VK_CONTROL.0)) < 0 }
}
