// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Forward original parent messages through the shared scrollbar without invoking application work.
use super::*;
use crate::windows_subclass::{callback, destroy_callback};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{Shell::DefSubclassProc, WindowsAndMessaging::*},
};

/// # Safety
/// Installed with this exact State registration on the original parent's creating UI thread.
pub(super) unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        // SAFETY: Final native destruction returns exactly this procedure's installed reference.
        return unsafe { destroy_callback::<State>(hwnd, Some(procedure), wparam, lparam, data) };
    }
    // SAFETY: The installed original registration retains State through this callback invocation.
    let state = unsafe { callback::<State>(data) };
    let admitted = state.active.get() && state.model.get().is_some();
    let mut inside = true;
    if message == WM_MOUSEWHEEL && admitted {
        let mut point = POINT { x: lparam.0 as i16 as i32, y: (lparam.0 >> 16) as i16 as i32 };
        // SAFETY: Map a copied screen point into only the original retained parent client.
        if !unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
            state.record(DesktopError::new(
                "native_scroll_wheel_point",
                windows::core::Error::from_thread(),
            ));
            return LRESULT(0);
        }
        inside = state.model.get().is_some_and(|m| {
            point.x >= m.bounds.left
                && point.x < m.bounds.right
                && point.y >= m.bounds.top
                && point.y < m.bounds.bottom
        });
    }
    if admitted && inside {
        if message == WM_KEYDOWN
            && let Some(key) =
                sugarsubstitute_ui::windows::scroll_port::ScrollKey::from_virtual_key(wparam.0)
        {
            match state.input.key(state, key) {
                Ok(true) => return LRESULT(0),
                Ok(false) => {}
                Err(cause) => {
                    state.record(cause.into());
                    return LRESULT(0);
                }
            }
        }
        match state.input.message(state, message, wparam, lparam) {
            Ok(true) if message != WM_MOUSEMOVE => return LRESULT(0),
            Ok(true) | Ok(false) => {}
            Err(cause) => {
                state.record(cause.into());
                return LRESULT(0);
            }
        }
    }
    // SAFETY: Preserve Tao and all unrelated native registrations before overlay publication.
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == WM_PAINT
        && let Err(cause) = state.paint(true)
    {
        state.record(cause);
    }
    result
}
