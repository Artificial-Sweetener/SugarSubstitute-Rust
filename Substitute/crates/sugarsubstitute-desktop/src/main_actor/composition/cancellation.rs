// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide when composed-window pointer gestures end without their own button release.
//!
//! WebView2 receives every forwarded press, so the page believes a button is held until it sees
//! the matching release. When the window is deactivated (including Alt+Tab), minimized or told
//! to cancel modes, the physical release may never reach this window; the host then ends the
//! native gesture and sends the page one release per button it saw held, so neither the canvas
//! nor a page drag stays captured. Capture moving to another window ends only a native gesture:
//! browser-owned popups may take capture during an ordinary page press.

use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        SIZE_MINIMIZED, WA_INACTIVE, WM_ACTIVATE, WM_CANCELMODE, WM_CAPTURECHANGED, WM_LBUTTONUP,
        WM_MBUTTONUP, WM_RBUTTONUP, WM_SIZE, WM_XBUTTONUP,
    },
};

/// Why active gestures end outside their own release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Interruption {
    /// Another window took capture, or capture was released outside a forwarded release.
    CaptureLost,
    /// The window was deactivated, minimized or told to cancel its modes.
    Cancelled,
}

/// Classify one host-window message; keyboard, IME and focus messages never interrupt.
///
/// Tao releases its own capture inside a forwarded button-up, after routing has already ended
/// that gesture, so the resulting capture change finds nothing left to cancel.
pub(super) fn interruption(
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    window: HWND,
) -> Option<Interruption> {
    match message {
        WM_CAPTURECHANGED if lparam.0 != window.0 as isize => Some(Interruption::CaptureLost),
        WM_CANCELMODE => Some(Interruption::Cancelled),
        WM_ACTIVATE if wparam.0 & 0xffff == WA_INACTIVE as usize => Some(Interruption::Cancelled),
        WM_SIZE if wparam.0 == SIZE_MINIMIZED as usize => Some(Interruption::Cancelled),
        _ => None,
    }
}

/// One synthesized button release for WebView2, in Win32 mouse-message terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Release {
    /// The `WM_*BUTTONUP` message kind.
    pub(super) kind: u32,
    /// The `MK_*` button state remaining after this release; modifiers are not reported.
    pub(super) keys: u16,
    /// `XBUTTON1` or `XBUTTON2` for an extra button, otherwise zero.
    pub(super) data: u32,
}

/// `MK_*` button bits paired with their release message and extra-button identity.
const BUTTONS: [(u16, u32, u32); 5] = [
    (0x0001, WM_LBUTTONUP, 0),
    (0x0002, WM_RBUTTONUP, 0),
    (0x0010, WM_MBUTTONUP, 0),
    (0x0020, WM_XBUTTONUP, 1),
    (0x0040, WM_XBUTTONUP, 2),
];

/// Release every button held in `keys`, one at a time, each reporting the buttons still held.
pub(super) fn releases(keys: u16) -> impl Iterator<Item = Release> {
    let mut held = keys & BUTTONS.iter().fold(0, |all, (bit, _, _)| all | bit);
    BUTTONS.into_iter().filter(move |(bit, _, _)| keys & bit != 0).map(move |(bit, kind, data)| {
        held &= !bit;
        Release { kind, keys: held, data }
    })
}

#[cfg(test)]
#[path = "cancellation/tests.rs"]
mod tests;
