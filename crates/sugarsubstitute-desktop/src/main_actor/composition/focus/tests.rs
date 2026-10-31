// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the host's keyboard-focus decisions without a browser.

use super::*;
use windows::Win32::UI::WindowsAndMessaging::{
    WM_CHAR, WM_IME_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_NOTIFY,
    WM_IME_SETCONTEXT, WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_SETFOCUS, WM_SYSKEYDOWN,
};

#[test]
fn every_press_focuses_an_unfocused_browser_and_leaves_a_focused_one_alone() {
    for press in [
        WM_LBUTTONDOWN,
        WM_LBUTTONDBLCLK,
        WM_MBUTTONDOWN,
        WM_MBUTTONDBLCLK,
        WM_RBUTTONDOWN,
        WM_RBUTTONDBLCLK,
        WM_XBUTTONDOWN,
        WM_XBUTTONDBLCLK,
    ] {
        assert!(requests_focus(press, false), "{press:#x}");
        assert!(!requests_focus(press, true), "{press:#x}");
    }
}

#[test]
fn movement_release_wheel_focus_keyboard_and_ime_messages_never_move_focus_first() {
    for message in [
        WM_MOUSEMOVE,
        WM_LBUTTONUP,
        WM_MOUSEWHEEL,
        WM_SETFOCUS,
        WM_KILLFOCUS,
        WM_KEYDOWN,
        WM_KEYUP,
        WM_SYSKEYDOWN,
        WM_CHAR,
        WM_IME_SETCONTEXT,
        WM_IME_STARTCOMPOSITION,
        WM_IME_COMPOSITION,
        WM_IME_ENDCOMPOSITION,
        WM_IME_NOTIFY,
        WM_IME_CHAR,
    ] {
        assert!(!requests_focus(message, false), "{message:#x}");
    }
}

#[test]
fn tab_traversal_leaving_the_page_wraps_inside_it() {
    assert_eq!(
        wrap(COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT),
        Some(COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT)
    );
    assert_eq!(
        wrap(COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS),
        Some(COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS)
    );
    assert_eq!(wrap(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC), None);
}
