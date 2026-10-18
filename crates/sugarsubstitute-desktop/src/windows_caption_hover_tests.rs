// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect exclusive hover transitions independently of live mouse timing or desktop capture.
use super::*;

/// Moving across peers dirties both paintings; delayed leave cannot erase the replacement.
#[test]
fn caption_hover_invalidates_previous_and_next_and_ignores_stale_leave() -> Result<(), DesktopError>
{
    // Pure transition identities borrow distinct live bytes and never enter Win32.
    let mut identities = [0u8; 3];
    let a = HWND((&raw mut identities[0]).cast());
    let b = HWND((&raw mut identities[1]).cast());
    let c = HWND((&raw mut identities[2]).cast());
    let hovered = Cell::new(HWND::default());
    let mut dirty = Vec::new();
    for (peer, enter, expected) in [
        (a, true, a),
        (b, true, b),
        (a, false, b),
        (c, true, c),
        (b, false, c),
        (c, false, HWND::default()),
    ] {
        change(&hovered, peer, enter, |peer| {
            dirty.push(peer);
            Ok(())
        })?;
        assert_eq!(hovered.get(), expected);
    }
    assert_eq!(dirty, [a, a, b, b, c, c]);
    Ok(())
}

/// A failure repainting the old peer must still schedule the new peer and retain one owner.
#[test]
fn caption_hover_observes_both_invalidations_after_native_failure() {
    // These distinct live addresses identify boundary fakes without creating native handles.
    let mut identities = [0u8; 2];
    let a = HWND((&raw mut identities[0]).cast());
    let b = HWND((&raw mut identities[1]).cast());
    let hovered = Cell::new(a);
    let mut dirty = Vec::new();
    assert!(
        change(&hovered, b, true, |peer| {
            dirty.push(peer);
            Err(DesktopError::message("caption_hover_fixture", "injected failure"))
        })
        .is_err()
    );
    assert_eq!(hovered.get(), b);
    assert_eq!(dirty, [a, b]);
}

/// Real hidden BUTTON peers exercise native bounds and input transitions without user input.
#[test]
fn native_caption_hover_clears_old_paint_and_capture_disable_hide_cancel_states()
-> Result<(), Box<dyn std::error::Error>> {
    use windows::{
        Win32::{
            Foundation::{HINSTANCE, POINT},
            UI::{
                Input::KeyboardAndMouse::EnableWindow,
                WindowsAndMessaging::{
                    BS_OWNERDRAW, CreateWindowExW, DestroyWindow, GetClassNameW, HMENU,
                    WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_POPUP, WS_VISIBLE,
                },
            },
        },
        core::w,
    };
    // SAFETY: This creating thread owns the hidden fixture parent and all of its children.
    let parent = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!(""),
            WS_POPUP,
            0,
            0,
            200,
            40,
            None,
            None,
            Some(HINSTANCE::default()),
            None,
        )
    }?;
    let result = (|| {
        let mut peers = Vec::new();
        for id in 1..=3 {
            // SAFETY: Real owner-drawn BUTTON children remain bounded by the hidden owned parent.
            peers.push(unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("BUTTON"),
                    w!("caption"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_OWNERDRAW as u32),
                    (id - 1) * 46,
                    1,
                    46,
                    32,
                    Some(parent),
                    Some(HMENU(id as usize as *mut _)),
                    Some(HINSTANCE::default()),
                    None,
                )
            }?);
        }
        let hovered = Cell::new(HWND::default());
        let point = POINT { x: 12, y: 12 };
        let inside = LPARAM(((point.y as u32) << 16 | point.x as u32) as isize);
        for peer in &peers {
            let mut name = [0u16; 32];
            // SAFETY: Query only the retained real native child and bounded class-name buffer.
            let length = unsafe { GetClassNameW(*peer, &mut name) };
            assert!(length > 0);
            assert!(
                String::from_utf16_lossy(&name[..length as usize]).eq_ignore_ascii_case("BUTTON")
            );
            let mut bounds = RECT::default();
            // SAFETY: This synchronous test retains every native peer until parent teardown.
            unsafe { GetClientRect(*peer, &raw mut bounds) }?;
            assert_eq!((bounds.right - bounds.left, bounds.bottom - bounds.top), (46, 32));
            let previous = hovered.get();
            child(&hovered, *peer, WM_MOUSEMOVE, WPARAM(0), inside)?;
            assert_eq!(hovered.get(), *peer);
            if !previous.is_invalid() {
                child(&hovered, previous, WM_MOUSELEAVE, WPARAM(0), LPARAM(0))?;
                assert_eq!(hovered.get(), *peer);
            }
        }
        let peer = peers[2];
        for (message, wparam, lparam) in [
            (WM_MOUSEMOVE, WPARAM(0), LPARAM(0xffff)),
            (WM_CAPTURECHANGED, WPARAM(0), LPARAM(peers[0].0 as isize)),
            (WM_CANCELMODE, WPARAM(0), LPARAM(0)),
            (WM_ENABLE, WPARAM(0), LPARAM(0)),
            (WM_SHOWWINDOW, WPARAM(0), LPARAM(0)),
            (WM_NCDESTROY, WPARAM(0), LPARAM(0)),
        ] {
            child(&hovered, peer, WM_MOUSEMOVE, WPARAM(0), inside)?;
            assert_eq!(hovered.get(), peer);
            child(&hovered, peer, message, wparam, lparam)?;
            assert!(hovered.get().is_invalid());
        }
        // SAFETY: Disable only the hidden owned child, then verify mouse traffic cannot reenter it.
        let _previous = unsafe { EnableWindow(peer, false) };
        child(&hovered, peer, WM_MOUSEMOVE, WPARAM(0), inside)?;
        assert!(hovered.get().is_invalid());
        Ok::<(), Box<dyn std::error::Error>>(())
    })();
    // SAFETY: Parent teardown retires all owned native BUTTONs on their creating test thread.
    let retired = unsafe { DestroyWindow(parent) };
    result?;
    retired.map_err(Into::into)
}
