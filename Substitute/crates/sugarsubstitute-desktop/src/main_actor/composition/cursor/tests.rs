// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise browser cursor selection without invoking a browser or changing the desktop.

use super::*;
use windows::Win32::UI::WindowsAndMessaging::{HTLEFT, IDC_IBEAM, IDC_SIZEWE, WM_MOUSEMOVE};

fn handle(value: usize) -> HCURSOR {
    HCURSOR(value as *mut std::ffi::c_void)
}

#[test]
fn system_cursor_ids_are_preferred_and_custom_handles_are_the_fallback() {
    let loaded = Cell::new(None);
    let handle_read = Cell::new(false);
    let system = choose::<()>(
        32513,
        |id| {
            loaded.set(Some(id));
            Some(handle(1))
        },
        || {
            handle_read.set(true);
            Ok(handle(2))
        },
    );
    assert_eq!(system, Ok(Some(handle(1))));
    assert_eq!(loaded.get(), Some(32513));
    assert!(!handle_read.get());

    let custom = choose::<()>(
        0,
        |_| {
            loaded.set(None);
            None
        },
        || Ok(handle(7)),
    );
    assert_eq!(custom, Ok(Some(handle(7))));
    assert_eq!(loaded.get(), Some(32513), "zero names no system cursor");
    let unknown = choose::<()>(0x7fff, |_| None, || Ok(handle(9)));
    assert_eq!(unknown, Ok(Some(handle(9))));
    let oversized = choose::<()>(
        0x1_0000,
        |_| {
            loaded.set(None);
            None
        },
        || Ok(handle(3)),
    );
    assert_eq!(oversized, Ok(Some(handle(3))));
    assert_eq!(loaded.get(), Some(32513), "oversized identifiers are not resources");
    assert_eq!(choose::<()>(0, |_| None, || Ok(HCURSOR::default())), Ok(None));
    assert_eq!(choose(0, |_| None, || Err("getter failed")), Err("getter failed"));
}

#[test]
fn reported_system_ids_resolve_to_the_shared_windows_cursors()
-> Result<(), Box<dyn std::error::Error>> {
    for shared in [IDC_IBEAM, IDC_SIZEWE] {
        let id = u16::try_from(shared.0 as usize)?;
        // SAFETY: Shared system cursors are process-wide and never destroyed by callers.
        let expected = unsafe { LoadCursorW(None, shared)? };
        assert_eq!(load_system(id), Some(expected));
    }
    Ok(())
}

#[test]
fn set_cursor_is_claimed_only_for_this_window_client_area() {
    let window = HWND(0x1234 as *mut std::ffi::c_void);
    let client = LPARAM(((WM_MOUSEMOVE as isize) << 16) | HTCLIENT as isize);
    assert!(claims(window, WPARAM(0x1234), client));
    assert!(!claims(window, WPARAM(0x5678), client));
    assert!(!claims(window, WPARAM(0x1234), LPARAM(HTLEFT as isize)));
}

#[test]
fn changed_cursors_apply_immediately_only_inside_or_under_capture() {
    let pointer = Pointer::default();
    assert_eq!(pointer.cursor(), None);
    assert_eq!(pointer.change(Some(handle(1)), false), None);
    assert_eq!(pointer.cursor(), Some(handle(1)));
    assert_eq!(pointer.change(Some(handle(2)), true), Some(handle(2)));
    assert!(pointer.enter());
    assert!(!pointer.enter());
    assert_eq!(pointer.change(Some(handle(3)), false), Some(handle(3)));
    assert_eq!(pointer.change(None, true), None);
    assert_eq!(pointer.cursor(), None);
    pointer.leave();
    assert_eq!(pointer.change(Some(handle(4)), false), None);
    assert!(pointer.enter());
}

#[test]
fn routed_canvas_hands_win_over_the_browser_cursor() {
    assert_eq!(shown(None, None), None);
    assert_eq!(shown(None, Some(handle(5))), Some(Shown::Browser(handle(5))));
    for intent in [CursorIntent::OpenHand, CursorIntent::ClosedHand] {
        assert_eq!(shown(Some(intent), Some(handle(5))), Some(Shown::Native(intent)));
        assert_eq!(shown(Some(intent), None), Some(Shown::Native(intent)));
    }
    let pointer = Pointer::default();
    assert!(!pointer.present(false));
    assert!(pointer.present(true));
    pointer.enter();
    assert!(pointer.present(false));
}
