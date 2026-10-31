// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise gesture interruption and synthesized page releases without a window.

use super::*;
use windows::Win32::UI::WindowsAndMessaging::{
    SIZE_MAXIMIZED, SIZE_RESTORED, WA_ACTIVE, WA_CLICKACTIVE, WM_CHAR, WM_IME_COMPOSITION,
    WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_KILLFOCUS, WM_SETFOCUS,
};

const WINDOW: HWND = HWND(0x1234 as *mut core::ffi::c_void);

fn classify(message: u32, wparam: usize, lparam: isize) -> Option<Interruption> {
    interruption(message, WPARAM(wparam), LPARAM(lparam), WINDOW)
}

#[test]
fn deactivation_minimize_and_cancel_mode_end_every_gesture() {
    assert_eq!(classify(WM_CANCELMODE, 0, 0), Some(Interruption::Cancelled));
    assert_eq!(classify(WM_ACTIVATE, WA_INACTIVE as usize, 0), Some(Interruption::Cancelled));
    // The high word reports minimized state; deactivation is the low word alone.
    assert_eq!(
        classify(WM_ACTIVATE, (1 << 16) | WA_INACTIVE as usize, 0),
        Some(Interruption::Cancelled)
    );
    assert_eq!(classify(WM_SIZE, SIZE_MINIMIZED as usize, 0), Some(Interruption::Cancelled));
}

#[test]
fn capture_moving_elsewhere_ends_only_a_native_gesture() {
    assert_eq!(classify(WM_CAPTURECHANGED, 0, 0), Some(Interruption::CaptureLost));
    assert_eq!(classify(WM_CAPTURECHANGED, 0, 0x5678), Some(Interruption::CaptureLost));
    assert_eq!(classify(WM_CAPTURECHANGED, 0, WINDOW.0 as isize), None);
}

#[test]
fn activation_restore_focus_keyboard_and_ime_messages_do_not_interrupt() {
    for (message, wparam) in [
        (WM_ACTIVATE, WA_ACTIVE as usize),
        (WM_ACTIVATE, WA_CLICKACTIVE as usize),
        (WM_SIZE, SIZE_RESTORED as usize),
        (WM_SIZE, SIZE_MAXIMIZED as usize),
        (WM_SETFOCUS, 0),
        (WM_KILLFOCUS, 0),
        (WM_KEYDOWN, 0x41),
        (WM_CHAR, 0x41),
        (WM_IME_STARTCOMPOSITION, 0),
        (WM_IME_COMPOSITION, 0),
    ] {
        assert_eq!(classify(message, wparam, 0), None, "{message:#x}");
    }
}

#[test]
fn releases_report_each_held_button_once_with_the_buttons_still_held() {
    // MK_LBUTTON | MK_SHIFT | MK_MBUTTON | MK_XBUTTON2.
    let released: Vec<_> = releases(0x0001 | 0x0004 | 0x0010 | 0x0040).collect();
    assert_eq!(
        released,
        [
            Release { kind: WM_LBUTTONUP, keys: 0x0050, data: 0 },
            Release { kind: WM_MBUTTONUP, keys: 0x0040, data: 0 },
            Release { kind: WM_XBUTTONUP, keys: 0, data: 2 },
        ]
    );
    assert_eq!(
        releases(0x0002 | 0x0020).collect::<Vec<_>>(),
        [
            Release { kind: WM_RBUTTONUP, keys: 0x0020, data: 0 },
            Release { kind: WM_XBUTTONUP, keys: 0, data: 1 },
        ]
    );
    assert_eq!(releases(0x0004 | 0x0008).count(), 0);
    assert_eq!(releases(0).count(), 0);
}
