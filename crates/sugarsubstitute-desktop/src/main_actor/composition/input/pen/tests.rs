// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise pen frame decoding and WebView2 forwarding decisions without a pen or browser.

use super::*;
use sugarsubstitute_canvas::ToolModifiers;
use windows::Win32::UI::{
    Input::Pointer::{POINTER_CHANGE_NONE, POINTER_FLAG_INRANGE, POINTER_INFO},
    WindowsAndMessaging::{PEN_FLAG_BARREL, PEN_MASK_NONE, WM_POINTERACTIVATE},
};

fn frame(flags: POINTER_FLAGS, change: POINTER_BUTTON_CHANGE_TYPE) -> POINTER_PEN_INFO {
    POINTER_PEN_INFO {
        pointerInfo: POINTER_INFO {
            pointerType: PT_PEN,
            pointerId: 7,
            pointerFlags: flags,
            ptPixelLocation: POINT { x: 1_250, y: 830 },
            ptPixelLocationRaw: POINT { x: 1_251, y: 829 },
            ptHimetricLocation: POINT { x: 33_000, y: 21_000 },
            ptHimetricLocationRaw: POINT { x: 33_010, y: 20_990 },
            ButtonChangeType: change,
            ..Default::default()
        },
        penFlags: 0,
        penMask: PEN_MASK_PRESSURE,
        pressure: 256,
        ..Default::default()
    }
}

#[test]
fn pen_messages_become_hover_contact_release_and_range_transitions() {
    let hover = frame(POINTER_FLAG_INRANGE, POINTER_CHANGE_NONE);
    let contact = frame(POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT, POINTER_CHANGE_NONE);
    assert_eq!(transition(WM_POINTERUPDATE, &hover), Some(Transition::Move { held: false }));
    assert_eq!(transition(WM_POINTERUPDATE, &contact), Some(Transition::Move { held: true }));
    let down = frame(POINTER_FLAG_INCONTACT, POINTER_CHANGE_FIRSTBUTTON_DOWN);
    assert_eq!(transition(WM_POINTERDOWN, &down), Some(Transition::Press(Button::Primary)));
    let up = frame(POINTER_FLAG_INRANGE, POINTER_CHANGE_FIRSTBUTTON_UP);
    assert_eq!(transition(WM_POINTERUP, &up), Some(Transition::Release(Button::Primary)));
    assert_eq!(transition(WM_POINTERENTER, &hover), Some(Transition::Enter));
    assert_eq!(transition(WM_POINTERLEAVE, &hover), Some(Transition::Leave));
    assert_eq!(transition(WM_POINTERCAPTURECHANGED, &hover), None);
    assert!(!consumed(WM_POINTERCAPTURECHANGED), "capture changes keep default processing");
    assert!(consumed(WM_POINTERUPDATE) && consumed(WM_POINTERDOWN) && consumed(WM_POINTERUP));
}

#[test]
fn barrel_contacts_are_secondary_and_other_pen_buttons_stay_extra() {
    let barrel = frame(POINTER_FLAG_INCONTACT, POINTER_CHANGE_SECONDBUTTON_DOWN);
    assert_eq!(transition(WM_POINTERDOWN, &barrel), Some(Transition::Press(Button::Secondary)));
    let unreported = frame(POINTER_FLAG_INCONTACT | POINTER_FLAG_SECONDBUTTON, POINTER_CHANGE_NONE);
    assert_eq!(
        transition(WM_POINTERDOWN, &unreported),
        Some(Transition::Press(Button::Secondary)),
        "the held flags decide when Windows reports no change"
    );
    let third = frame(POINTER_FLAG_INCONTACT, POINTER_CHANGE_THIRDBUTTON_UP);
    assert_eq!(transition(WM_POINTERUP, &third), Some(Transition::Release(Button::Extra)));
}

#[test]
fn samples_normalize_reported_pressure_and_default_to_full_without_it() {
    let modifiers = ToolModifiers { shift: false, ctrl: true, alt: false };
    let mut pen = frame(POINTER_FLAG_INCONTACT, POINTER_CHANGE_NONE);
    let sampled = sample(&pen, POINT { x: 250, y: 30 }, modifiers);
    assert_eq!(
        sampled,
        PointerSample {
            position: [250.0, 30.0],
            kind: PointerKind::Pen,
            pressure: Pressure::from_range(256, 1024),
            eraser: false,
            modifiers,
        }
    );
    assert_eq!(sampled.pressure.get(), 0.25);
    pen.pressure = 4_000;
    assert_eq!(sample(&pen, POINT::default(), modifiers).pressure, Pressure::FULL, "clamped");
    pen.pressure = 0;
    assert_eq!(sample(&pen, POINT::default(), modifiers).pressure.get(), 0.0, "hover");
    pen.penMask = PEN_MASK_NONE;
    assert_eq!(sample(&pen, POINT::default(), modifiers).pressure, Pressure::FULL);
}

#[test]
fn the_eraser_end_and_an_inverted_pen_erase_but_the_barrel_does_not() {
    let mut pen = frame(POINTER_FLAG_INCONTACT, POINTER_CHANGE_NONE);
    for (flags, eraser) in [
        (PEN_FLAG_ERASER, true),
        (PEN_FLAG_INVERTED, true),
        (PEN_FLAG_INVERTED | PEN_FLAG_ERASER, true),
        (PEN_FLAG_BARREL, false),
        (0, false),
    ] {
        pen.penFlags = flags;
        assert_eq!(sample(&pen, POINT::default(), ToolModifiers::NONE).eraser, eraser, "{flags}");
    }
}

#[test]
fn only_mouse_messages_promoted_from_a_pen_are_recognized() {
    assert!(promoted_from_pen(0xff51_5700_u32 as i32 as isize));
    assert!(promoted_from_pen(0xff51_5701_u32 as isize), "pen with a low cursor identity");
    assert!(!promoted_from_pen(0xff51_5780_u32 as isize), "touch keeps its promoted path");
    assert!(!promoted_from_pen(0));
    assert!(!promoted_from_pen(0x1234));
}

#[test]
fn the_page_leaves_when_the_canvas_claims_the_pen_and_reenters_before_regaining_it() {
    assert_eq!(page_events(WM_POINTERENTER, true, false), ([Some(WM_POINTERENTER), None], true));
    assert_eq!(
        page_events(WM_POINTERUPDATE, true, true),
        ([None, Some(WM_POINTERUPDATE)], true),
        "an ordinary forwarded update"
    );
    assert_eq!(page_events(WM_POINTERUPDATE, false, true), ([Some(WM_POINTERLEAVE), None], false));
    assert_eq!(page_events(WM_POINTERDOWN, false, false), ([None, None], false), "withheld");
    assert_eq!(
        page_events(WM_POINTERUPDATE, true, false),
        ([Some(WM_POINTERENTER), Some(WM_POINTERUPDATE)], true)
    );
    assert_eq!(page_events(WM_POINTERLEAVE, true, true), ([Some(WM_POINTERLEAVE), None], false));
    assert_eq!(page_events(WM_POINTERLEAVE, true, false), ([None, None], false), "already left");
    assert_eq!(
        page_events(WM_POINTERACTIVATE, true, true),
        ([None, Some(WM_POINTERACTIVATE)], true)
    );
}

#[test]
fn page_pointers_move_pixels_and_display_into_client_space_and_keep_himetric() {
    let pen = frame(POINTER_FLAG_INCONTACT, POINTER_CHANGE_FIRSTBUTTON_DOWN);
    let origin = POINT { x: 1_000, y: 800 };
    let device = RECT { left: 0, top: 0, right: 50_000, bottom: 30_000 };
    let display = RECT { left: -1_920, top: 0, right: 1_920, bottom: 2_160 };
    let page = page_pointer(pen, origin, [device, display]);
    let pointer = &page.pen.pointerInfo;
    assert_eq!(pointer.ptPixelLocation, POINT { x: 250, y: 30 });
    assert_eq!(pointer.ptPixelLocationRaw, POINT { x: 251, y: 29 });
    assert_eq!(pointer.ptHimetricLocation, pen.pointerInfo.ptHimetricLocation);
    assert_eq!(pointer.ptHimetricLocationRaw, pen.pointerInfo.ptHimetricLocationRaw);
    assert_eq!(page.device, device);
    assert_eq!(page.display, RECT { left: -2_920, top: -800, right: 920, bottom: 1_360 });
    assert_eq!((page.pen.pressure, page.pen.penMask), (256, PEN_MASK_PRESSURE));
    assert_eq!(client(POINT { x: 990, y: 790 }, origin), POINT { x: -10, y: -10 });
}
