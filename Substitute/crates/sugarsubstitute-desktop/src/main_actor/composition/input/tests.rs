// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise native mouse decoding without invoking a browser or changing the desktop.

use super::*;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_NON_CLIENT_REGION_KIND, COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HTCAPTION, WM_CHAR, WM_DEADCHAR, WM_IME_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
    WM_IME_NOTIFY, WM_IME_SETCONTEXT, WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
    WM_SETFOCUS, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_UNICHAR,
};

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

/// While the page sits below the caption strip, pointer messages move into page pixels, and a
/// leave stays at the page's top-left: the client origin above the page is outside it, which
/// WebView2 rejects, and that rejection closed the window whenever the pointer left it.
#[test]
fn a_leave_stays_inside_a_page_below_the_caption_strip() {
    let origin = POINT { x: 0, y: 60 };
    assert_eq!(
        page_input_point(WM_MOUSEMOVE, POINT { x: 300, y: 200 }, origin),
        POINT { x: 300, y: 140 }
    );
    assert_eq!(page_input_point(WM_MOUSELEAVE, POINT::default(), origin), POINT::default());
}

/// The subclass consumes only client-area `WM_SETCURSOR`; these reach Tao and the browser's own
/// focus window exactly as Windows sent them, without forwarding, focus moves or cancellation.
#[test]
fn keyboard_ime_and_focus_messages_pass_through_untouched() {
    let window = HWND(0x1234 as *mut core::ffi::c_void);
    for message in [
        WM_KEYDOWN,
        WM_KEYUP,
        WM_SYSKEYDOWN,
        WM_SYSKEYUP,
        WM_CHAR,
        WM_DEADCHAR,
        WM_SYSCHAR,
        WM_UNICHAR,
        WM_IME_SETCONTEXT,
        WM_IME_STARTCOMPOSITION,
        WM_IME_COMPOSITION,
        WM_IME_ENDCOMPOSITION,
        WM_IME_NOTIFY,
        WM_IME_CHAR,
        WM_SETFOCUS,
        WM_KILLFOCUS,
    ] {
        for (wparam, lparam) in [(0, 0), (0x41, 0x001e_0001), (usize::MAX, -1)] {
            let (wparam, lparam) = (WPARAM(wparam), LPARAM(lparam));
            assert_eq!(mouse(message, wparam, lparam), None, "{message:#x}");
            assert_eq!(
                cancellation::interruption(message, wparam, lparam, window),
                None,
                "{message:#x}"
            );
            assert!(!focus::requests_focus(message, false), "{message:#x}");
        }
    }
}

/// Only the page's caption moves the window; window buttons stay with the host's own controls.
#[test]
fn only_the_page_caption_region_becomes_a_window_caption() {
    assert_eq!(
        page::caption_hit(COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION),
        Some(LRESULT(HTCAPTION as isize))
    );
    // WebView2's nowhere, client, minimize, maximize and close region kinds.
    for kind in [0, 1, 8, 9, 20] {
        assert_eq!(page::caption_hit(COREWEBVIEW2_NON_CLIENT_REGION_KIND(kind)), None, "{kind}");
    }
}

/// The client row where the real caption layout starts the page in a 1280 by 800 logical
/// window at 175%, the proof desktop's 168 DPI, with or without the update line.
fn page_top_at_175(line: bool) -> Result<(u32, [u32; 2]), Box<dyn std::error::Error>> {
    use crate::main_updates::{CaptionUpdates, Notice};
    use sugarsubstitute_contracts::{execution::Generation, lifecycle, startup::LaunchId};
    let scale = 1.75;
    let client = [2240_u32, 1400_u32];
    let mut caption = CaptionUpdates::default();
    if line {
        caption.notify(Notice::Updated { version: "0.2.0".to_owned() });
    }
    let identity = crate::MainIdentity {
        run: lifecycle::RunKey {
            owner: lifecycle::CoordinatorKey {
                namespace: lifecycle::NamespaceId::new([1; 32]),
                epoch: lifecycle::CoordinatorEpoch::new([2; 16])?,
            },
            instance: lifecycle::InstanceId::new(1)?,
            generation: lifecycle::RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    let retry = crate::main_backend::Recovery::new(identity)
        .presentation(crate::MainRecoveryLabels { retry: "Retry", retry_ready: "Ready" });
    let labels = crate::MainCaptionLabels {
        minimize: "min",
        maximize: "max",
        restore: "restore",
        close: "close",
    };
    let width = f64::from(client[0]) / scale;
    let view = caption.view(sugarsubstitute_contracts::localization::messages("en"));
    let measure = |text: &str| Ok::<_, std::convert::Infallible>(text.chars().count() as f64 * 7.0);
    let controls =
        crate::main_caption::caption_controls(width, false, labels, retry, &view, measure)?;
    Ok((crate::main_caption::page_top(width, &controls, scale), client))
}

/// At 175% the browser is placed at the real layout's page offset, and a pointer over any page
/// pixel reaches WebView2 as that same page pixel, while the line shows and after Dismiss.
#[test]
fn page_input_lands_on_the_page_pixel_under_the_pointer_at_175_percent()
-> Result<(), Box<dyn std::error::Error>> {
    let mut tops = Vec::new();
    for line in [true, false] {
        let (top, [width, height]) = page_top_at_175(line)?;
        tops.push(top);
        let bounds = crate::main_actor::frame::browser_bounds(width, height, top);
        let position = bounds.position.to_physical::<i32>(1.75);
        let size = bounds.size.to_physical::<i32>(1.75);
        let origin = POINT { x: position.x, y: position.y };
        // The tab bar's new-tab button, the avatar, and the page's first and last pixels.
        let targets = [
            POINT { x: 432, y: 33 },
            POINT { x: 55, y: 52 },
            POINT { x: 0, y: 0 },
            POINT { x: size.width - 1, y: size.height - 1 },
        ];
        for page in targets {
            let client = POINT { x: page.x + origin.x, y: page.y + origin.y };
            for kind in [WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP] {
                assert_eq!(page_input_point(kind, client, origin), page, "line={line}");
            }
        }
        assert_eq!(page_input_point(WM_MOUSELEAVE, POINT::default(), origin), POINT::default());
    }
    assert_eq!(tops, [60, 0], "the line moves the page down 60 pixels; Dismiss returns it");
    Ok(())
}
