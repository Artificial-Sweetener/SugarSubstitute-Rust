// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native input routing before testing document operations behind that routing.
use super::{Hidden, TestResult, UiApartment, index, source};
use windows::Win32::{
    Foundation::{LPARAM, POINT},
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{HTCLIENT, SendMessageW, WM_NCHITTEST},
};

/// Observe actual glyph coordinates, independently of the tail-follow predicate.
fn character_point(hwnd: windows::Win32::Foundation::HWND, character: i32) -> POINT {
    let mut point = POINT::default();
    // SAFETY: The exact native child synchronously writes this point for a document character.
    unsafe {
        SendMessageW(
            hwnd,
            windows::Win32::UI::Controls::EM_POSFROMCHAR,
            Some(windows::Win32::Foundation::WPARAM((&raw mut point) as usize)),
            Some(LPARAM(character as isize)),
        );
    }
    point
}

fn packed(point: POINT) -> LPARAM {
    LPARAM(((point.x as u16 as u32) | ((point.y as u16 as u32) << 16)) as isize)
}

/// Exercise the native input procedure with real layout, focus, capture and continuing publication.
#[test]
fn native_console_pointer_scroll_selection_survives_continuing_output() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::native_console_pointer_scroll_selection_survives_continuing_output"
        ),
        qualify_input,
    )
}

fn qualify_input() -> TestResult {
    use super::super::{observe, publish};
    use crate::{
        console::update::Documents, windows_controls::console::messages::Messages,
        windows_text as engine,
    };
    use windows::Win32::{
        Foundation::WPARAM,
        UI::{Input::KeyboardAndMouse::GetFocus, WindowsAndMessaging::*},
    };
    let mut apartment = UiApartment::enter()?;
    let rows: Vec<_> =
        (1..=110).map(|id| (id, format!("Record {id:03} readable output"))).collect();
    let refs: Vec<_> = rows.iter().map(|(id, text)| (*id, text.as_str())).collect();
    let (document, _frame, _release) = index(&source(1, &refs[..80])?)?;
    let mut owner = Hidden::new(document, &apartment)?;
    let hwnd = owner.hwnd.ok_or("native child absent")?;
    let mut documents = Documents::new(owner.document.take().ok_or("document absent")?);
    publish(hwnd, &mut documents, true)?;
    let control = Messages(hwnd);
    let (paint_frame, _paint_release) = super::native_alpha_tests::frame()?;
    let mut storage = None;
    let target = crate::native_storage::Dib::for_frame(&mut storage, &paint_frame)?;
    let engine = engine::find(hwnd)?;
    let mut paint = |documents: &Documents| -> TestResult {
        engine.prepare(
            &paint_frame,
            Some(documents.shared().ok_or("shared document absent")?),
            windows::Win32::Foundation::RECT { left: 25, top: 20, right: 325, bottom: 140 },
            true,
            1.0,
        )?;
        engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
        Ok(())
    };
    paint(&documents)?;
    // SAFETY: Both exact fixture windows belong exclusively to the owned inactive desktop.
    unsafe {
        let _previous = ShowWindow(GetAncestor(hwnd, GA_PARENT), SW_SHOWNOACTIVATE);
        let _previous = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _previous = windows::Win32::UI::Input::KeyboardAndMouse::SetActiveWindow(GetAncestor(
            hwnd, GA_PARENT,
        ));
    }
    let last = documents.current().ok_or("document absent")?.units as i32 - 2;
    let tail = character_point(hwnd, last);
    assert!((0..120).contains(&tail.y), "last character is not visible: {tail:?}");
    let before = observe(hwnd, true)?;
    // SAFETY: Exercise focus acquisition using a real click on this private-desktop text child.
    unsafe {
        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(packed(POINT { x: 50, y: 50 })));
        SendMessageW(hwnd, WM_LBUTTONUP, Some(WPARAM(0)), Some(packed(POINT { x: 50, y: 50 })));
        assert_eq!(GetFocus(), hwnd);
    }
    engine::find(hwnd)?.check()?;
    let mut wheel_point = POINT { x: 100, y: 50 };
    // SAFETY: Convert only the original child point to the wheel message's screen coordinates.
    unsafe { ClientToScreen(hwnd, &mut wheel_point) }.ok()?;
    // SAFETY: Send a wheel-up gesture through the actual child procedure on the private desktop.
    unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSEWHEEL,
            Some(WPARAM((120u32 << 16) as usize)),
            Some(packed(wheel_point)),
        );
    }
    // SAFETY: Dispatch only this test thread's pending native messages without waiting.
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            DispatchMessageW(&message);
        }
    }
    let scrolled = observe(hwnd, true)?;
    assert!(
        scrolled.first < before.first,
        "wheel did not expose earlier text: {} -> {}",
        before.first,
        scrolled.first
    );
    assert!(character_point(hwnd, last).y >= 120, "wheel did not move the actual glyph viewport");
    let (next, _next_frame, _next_release) = index(&source(2, &refs[..90])?)?;
    documents.stage(next)?;
    publish(hwnd, &mut documents, true)?;
    paint(&documents)?;
    assert_eq!(observe(hwnd, true)?.first, scrolled.first, "output stole scrollback");
    let start = scrolled.first + 29;
    let end = start + 6;
    let mut from = character_point(hwnd, start);
    let mut to = character_point(hwnd, end);
    from.y += 4;
    to.y += 4;
    assert!((0..288).contains(&from.x) && (0..120).contains(&from.y), "{from:?}");
    assert!((0..288).contains(&to.x) && (0..120).contains(&to.y), "{to:?}");
    // SAFETY: Click and Shift-click select through the native procedure on the private desktop.
    unsafe {
        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(packed(from)));
        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(packed(from)));
        paint(&documents)?;
        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(5)), Some(packed(to)));
        SendMessageW(hwnd, WM_LBUTTONUP, Some(WPARAM(0)), Some(packed(to)));
        assert_eq!(GetFocus(), hwnd, "pointer input did not focus the text host");
    }
    engine::find(hwnd)?.check()?;
    let selected = control.selection();
    assert_eq!((selected.cpMin, selected.cpMax), (start, end), "pointer selection differs");
    let (next, _last_frame, _last_release) = index(&source(3, &refs[..100])?)?;
    documents.stage(next)?;
    publish(hwnd, &mut documents, true)?;
    paint(&documents)?;
    let retained = control.selection();
    assert_eq!((retained.cpMin, retained.cpMax), (start, end), "output replaced selection");
    assert_eq!(observe(hwnd, true)?.first, scrolled.first);
    control.select(end, end)?;
    // SAFETY: Native scrollbar bottom gesture on the original retained child.
    unsafe {
        SendMessageW(hwnd, WM_VSCROLL, Some(WPARAM(SB_BOTTOM.0 as usize)), None);
    }
    let last = documents.current().ok_or("document absent")?.units as i32 - 2;
    assert!((0..120).contains(&character_point(hwnd, last).y));
    let (next, _follow_frame, _follow_release) = index(&source(4, &refs)?)?;
    documents.stage(next)?;
    publish(hwnd, &mut documents, true)?;
    paint(&documents)?;
    let last = documents.current().ok_or("document absent")?.units as i32 - 2;
    assert!((0..120).contains(&character_point(hwnd, last).y), "tail-follow did not resume");
    // SAFETY: Drag the real collapsed scrollbar thumb across the private child's client track.
    unsafe {
        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(packed(POINT { x: 295, y: 100 })));
        SendMessageW(hwnd, WM_MOUSEMOVE, Some(WPARAM(1)), Some(packed(POINT { x: 295, y: 12 })));
        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(packed(POINT { x: 295, y: 12 })));
    }
    assert_eq!(observe(hwnd, true)?.first, 0, "thumb did not reach document start");
    assert!((0..120).contains(&character_point(hwnd, 0).y));
    // SAFETY: Return the same retained thumb to the bottom of its client track.
    unsafe {
        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(packed(POINT { x: 295, y: 20 })));
        SendMessageW(hwnd, WM_MOUSEMOVE, Some(WPARAM(1)), Some(packed(POINT { x: 295, y: 108 })));
        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(packed(POINT { x: 295, y: 108 })));
    }
    assert!((0..120).contains(&character_point(hwnd, last).y), "thumb cannot reach last line");
    target.close()?;
    owner.close()?;
    drop((owner, documents, engine, paint_frame));
    // SAFETY: Every fixture window, native document and callback has retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}

/// A console must claim mouse input before Windows can deliver selection or wheel gestures.
#[test]
fn native_console_hit_test_claims_text_and_scrollbar_input() -> TestResult {
    let mut apartment = UiApartment::enter()?;
    let (document, _frame, _release) = index(&source(1, &[(1, "input target")])?)?;
    let mut owner = Hidden::new(document, &apartment)?;
    let hwnd = owner.hwnd.ok_or("native child absent")?;
    for (x, y) in [(20, 20), (150, 60), (295, 60)] {
        let mut point = POINT { x, y };
        // SAFETY: Only the owned hidden child's coordinate conversion and hit-test procedure run.
        let hit = unsafe {
            ClientToScreen(hwnd, &mut point).ok()?;
            SendMessageW(
                hwnd,
                WM_NCHITTEST,
                None,
                Some(LPARAM(((point.x as u16 as u32) | ((point.y as u16 as u32) << 16)) as isize)),
            )
        };
        assert_eq!(hit.0, HTCLIENT as isize, "console rejected input at ({x}, {y})");
    }
    owner.close()?;
    drop(owner);
    // SAFETY: The exact native child, parent, and services have retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}
