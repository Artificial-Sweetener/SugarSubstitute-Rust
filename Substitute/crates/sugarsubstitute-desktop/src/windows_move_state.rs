// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe movement of the retained desktop HWND on its creating UI thread.
//!
//! A positive observation requires both the native move-loop flag and the exact
//! owned HWND. This query neither pumps messages nor changes capture or movement.

use crate::DesktopError;
use std::{ffi::c_void, mem::size_of};
use tao::{platform::windows::WindowExtWindows, window::Window};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GUI_INMOVESIZE, GUITHREADINFO, GetGUIThreadInfo, GetWindowThreadProcessId,
    },
};

/// Observe the owned window's actual native movement state without retaining its handle.
///
/// Call only on the window's creating UI thread while the caller retains the Tao
/// window. Native identity/query failures preserve their underlying Win32 cause.
/// A foreign window moving on the same thread does not qualify this surface.
pub(crate) fn is_moving(window: &Window) -> Result<bool, DesktopError> {
    let hwnd = HWND(window.hwnd() as *mut c_void);
    let mut state =
        GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..GUITHREADINFO::default() };
    // SAFETY: The caller retains this thread's window. The query writes only the
    // correctly sized local structure and uses the exact owned HWND's thread ID.
    unsafe {
        let thread = GetWindowThreadProcessId(hwnd, None);
        if thread == 0 {
            return Err(DesktopError::new(
                "query_native_move_thread",
                windows::core::Error::from_thread(),
            ));
        }
        GetGUIThreadInfo(thread, &raw mut state)
            .map_err(|error| DesktopError::new("query_native_move_state", error))?;
    }
    Ok(matches_owned_move(hwnd, &state))
}

/// Accept only an actual move-loop flag associated with a valid owned window identity.
fn matches_owned_move(hwnd: HWND, state: &GUITHREADINFO) -> bool {
    !hwnd.is_invalid() && state.flags.contains(GUI_INMOVESIZE) && state.hwndMoveSize == hwnd
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::GUI_INMENUMODE;

    /// A matching handle without movement, or a moving foreign handle, cannot qualify a frame.
    #[test]
    fn native_move_evidence_requires_owned_handle_and_move_loop_flag() {
        let mut owned_identity = 0u8;
        let mut foreign_identity = 0u8;
        let owned = HWND((&raw mut owned_identity).cast());
        let foreign = HWND((&raw mut foreign_identity).cast());
        let mut state = GUITHREADINFO { hwndMoveSize: owned, ..GUITHREADINFO::default() };
        assert!(!matches_owned_move(owned, &state));
        state.flags = GUI_INMENUMODE;
        assert!(!matches_owned_move(owned, &state));
        state.flags |= GUI_INMOVESIZE;
        assert!(matches_owned_move(owned, &state));
        state.hwndMoveSize = foreign;
        assert!(!matches_owned_move(owned, &state));
        state.hwndMoveSize = HWND::default();
        assert!(!matches_owned_move(HWND::default(), &state));
    }
}
