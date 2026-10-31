// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise native mouse decoding without invoking a browser or changing the desktop.

use super::*;
use windows::Win32::UI::WindowsAndMessaging::WM_SIZE;

#[test]
fn mouse_messages_preserve_signed_coordinates_buttons_and_wheel_units() {
    let point = LPARAM(0xfffd_fffe_u32 as isize);
    let movement = mouse(WM_MOUSEMOVE, WPARAM(0x0005), point);
    assert_eq!(
        movement,
        Some(Mouse { kind: WM_MOUSEMOVE, keys: 5, data: 0, point: [-2, -3], screen: false })
    );
    let wheel = mouse(
        WM_MOUSEWHEEL,
        WPARAM((u16::from_ne_bytes((-120_i16).to_ne_bytes()) as usize) << 16 | 4),
        point,
    );
    assert_eq!(
        wheel,
        Some(Mouse {
            kind: WM_MOUSEWHEEL,
            keys: 4,
            data: (-120_i32) as u32,
            point: [-2, -3],
            screen: true,
        })
    );
    let extra = mouse(WM_XBUTTONDOWN, WPARAM((2 << 16) | 1), point);
    assert_eq!(extra.map(|value| (value.keys, value.data)), Some((1, 2)));
    assert_eq!(mouse(WM_XBUTTONDOWN, WPARAM(3 << 16), point), None);
    assert_eq!(mouse(WM_MOUSEMOVE, WPARAM(0x80), point), None);
}

#[test]
fn mouse_leave_has_no_stale_pointer_data_and_other_messages_are_not_forwarded() {
    assert_eq!(
        mouse(WM_MOUSELEAVE, WPARAM(123), LPARAM(456)),
        Some(Mouse { kind: WM_MOUSELEAVE, keys: 0, data: 0, point: [0, 0], screen: false })
    );
    assert_eq!(mouse(WM_SIZE, WPARAM(0), LPARAM(0)), None);
}
