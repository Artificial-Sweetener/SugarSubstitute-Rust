// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hit-test the real Main caption on a private desktop: the strip above a page that starts
//! below it, and the message line's words, move the window; the caption's buttons stay
//! clickable. Every check goes through Windows' own hit test or the window's procedure chain.
use super::tests::{LABELS, TestResult, WIDTH, identity, peer_bounds};
use super::*;
use crate::{
    MainRecoveryLabels,
    main_backend::Recovery,
    main_caption::caption_controls,
    main_updates::{CaptionUpdates, Notice},
};
use sugarsubstitute_contracts::localization::messages;
use tao::{dpi::LogicalSize, event_loop::EventLoopBuilder, window::WindowBuilder};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{GetDlgItem, SendMessageW},
};
/// Ask the real window for its hit at client point (`x`, `y`), as Windows does for the pointer.
fn hit_at(parent: HWND, x: i32, y: i32) -> Result<isize, Box<dyn std::error::Error>> {
    let mut point = windows::Win32::Foundation::POINT { x, y };
    // SAFETY: Map this test window's client point to the screen; no input or paint occurs.
    if !unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(parent, &raw mut point) }.as_bool() {
        return Err("client origin unavailable".into());
    }
    let lparam = LPARAM(((point.y as u16 as isize) << 16) | point.x as u16 as isize);
    // SAFETY: A synchronous hit test on this test's own window on its private desktop.
    Ok(unsafe {
        SendMessageW(
            parent,
            windows::Win32::UI::WindowsAndMessaging::WM_NCHITTEST,
            Some(WPARAM(0)),
            Some(lparam),
        )
    }
    .0)
}

/// A default procedure below the Main caption that answers every client hit with `HTNOWHERE`,
/// the answer the proof desktop's frameless Main window gave over the strip and the tab row.
unsafe extern "system" fn nowhere(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{HTCLIENT, HTNOWHERE};
    // SAFETY: Forward this test window's live message through its own subclass chain.
    let hit = unsafe { windows::Win32::UI::Shell::DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == windows::Win32::UI::WindowsAndMessaging::WM_NCHITTEST
        && hit.0 == HTCLIENT as isize
    {
        return windows::Win32::Foundation::LRESULT(HTNOWHERE as isize);
    }
    hit
}

/// The strip above a page that starts below it moves the window: a real hit test through the
/// Main window's whole procedure chain answers it with `HTCAPTION`, whether the frameless
/// window's default answer there is client or nowhere, and the page below keeps its own answer.
fn strip_hit_body() -> TestResult {
    for answers_nowhere in [false, true] {
        strip_hit_case(answers_nowhere)?;
    }
    Ok(())
}

fn strip_hit_case(answers_nowhere: bool) -> TestResult {
    use windows::Win32::UI::WindowsAndMessaging::HTCAPTION;
    let identity = identity()?;
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    tao::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);
    let events = builder.build();
    // Built as the Main window is: frameless, resizable and transparent.
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("Main caption strip qualification")
            .with_inner_size(LogicalSize::new(f64::from(WIDTH), 600.0))
            .with_decorations(false)
            .with_resizable(true)
            .with_transparent(true)
            .with_visible(true)
            .build(&events)?,
    );
    let parent = HWND(window.hwnd() as *mut c_void);
    if answers_nowhere {
        // SAFETY: Installed below the caption on this test's own window, which it outlives.
        if !unsafe { windows::Win32::UI::Shell::SetWindowSubclass(parent, Some(nowhere), 1, 0) }
            .as_bool()
        {
            return Err("default-procedure stand-in not installed".into());
        }
    }
    // The Main window's own material: Mica Alt over a client-wide glass frame.
    let presenter = crate::windows::Presenter::new_main(window.clone(), true)?;
    let retry = Recovery::new(identity)
        .presentation(MainRecoveryLabels { retry: "Retry", retry_ready: "Retry ready" });
    let measure = |text: &str| crate::windows_callbacks::caption::measure_text(parent, text);
    let view = CaptionUpdates::default().view(messages("en"));
    let layout = caption_controls(f64::from(WIDTH), false, LABELS, retry, &view, measure)?;
    let mut controls = Controls::new(
        window.clone(),
        events.create_proxy(),
        &Specification::buttons(&layout),
        PaintMode::NativeButtons(identity),
        None,
    )?;
    controls.configure_caption(true, presenter.material());
    controls.initialize()?;
    let scale = window.scale_factor();
    let top = (34.0 * scale).ceil() as i32;
    let caption = HTCAPTION as isize;
    let result = (|| -> TestResult {
        let (x, strip_y, page_y) = ((300.0 * scale) as i32, top / 2, top + top / 2);
        let filling = hit_at(parent, x, strip_y)?;
        controls.set_page_top(top as u32);
        let strip = hit_at(parent, x, strip_y)?;
        let page = hit_at(parent, x, page_y)?;
        controls.set_page_top(0);
        let dismissed = hit_at(parent, x, strip_y)?;
        assert_eq!(strip, caption, "strip with default answer {filling}");
        assert_eq!(page, filling, "the page below the strip keeps its own answer");
        assert_eq!(dismissed, filling, "after Dismiss the page's row answers for itself");
        Ok(())
    })();
    let shutdown = controls.shutdown();
    drop(controls);
    drop(presenter);
    drop(window);
    result?;
    shutdown?;
    Ok(())
}

#[test]
fn the_strip_above_the_page_answers_a_real_hit_test_as_caption() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::the_strip_above_the_page_answers_a_real_hit_test_as_caption"),
        strip_hit_body,
    )
}

/// The window Windows' own hit test finds under client point (`x`, `y`) of `parent`.
fn window_at(parent: HWND, x: i32, y: i32) -> Result<HWND, Box<dyn std::error::Error>> {
    let mut point = windows::Win32::Foundation::POINT { x, y };
    // SAFETY: Map this test window's client point to the screen; no input or paint occurs.
    if !unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(parent, &raw mut point) }.as_bool() {
        return Err("client origin unavailable".into());
    }
    // SAFETY: Hit-tests this thread's private desktop, sending WM_NCHITTEST to its own windows.
    Ok(unsafe { windows::Win32::UI::WindowsAndMessaging::WindowFromPoint(point) })
}

/// Pressing on the message line's words drags the window like the strip around them, while
/// "What's new" and "Dismiss" stay clickable: Windows' own hit test passes the line through
/// to the Main window, which answers caption, and stops at the two buttons.
fn line_drag_body() -> TestResult {
    use windows::Win32::UI::WindowsAndMessaging::HTCAPTION;
    let identity = identity()?;
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    tao::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);
    let events = builder.build();
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("Main caption line qualification")
            .with_inner_size(LogicalSize::new(f64::from(WIDTH), 600.0))
            .with_decorations(false)
            .with_resizable(true)
            .with_transparent(true)
            .with_visible(true)
            .build(&events)?,
    );
    let parent = HWND(window.hwnd() as *mut c_void);
    let presenter = crate::windows::Presenter::new_main(window.clone(), true)?;
    let retry = Recovery::new(identity)
        .presentation(MainRecoveryLabels { retry: "Retry", retry_ready: "Retry ready" });
    let measure = |text: &str| crate::windows_callbacks::caption::measure_text(parent, text);
    let mut caption = CaptionUpdates::default();
    caption.notify(Notice::update(crate::UpdateNotice::Updated {
        version: "0.2.0".into(),
        changelog: url::Url::parse("https://github.com/example/releases/tag/v0.2.0").ok(),
    }));
    let view = caption.view(messages("en"));
    let width = window.inner_size().to_logical::<f64>(window.scale_factor()).width;
    let layout = caption_controls(width, false, LABELS, retry, &view, measure)?;
    let mut controls = Controls::new(
        window.clone(),
        events.create_proxy(),
        &Specification::buttons(&layout),
        PaintMode::NativeButtons(identity),
        None,
    )?;
    controls.configure_caption(true, presenter.material());
    controls.initialize()?;
    let result = (|| -> TestResult {
        controls.present_caption(&layout, &view)?;
        controls.set_page_top(crate::main_caption::page_top(width, &layout, window.scale_factor()));
        let centre = |id: u16| -> Result<(i32, i32), Box<dyn std::error::Error>> {
            let bounds = peer_bounds(parent, id)?;
            Ok(((bounds.left + bounds.right) / 2, (bounds.top + bounds.bottom) / 2))
        };
        let (x, y) = centre(crate::main_updates::LINE)?;
        assert_eq!(window_at(parent, x, y)?, parent, "the line's words fall through to the strip");
        assert_eq!(hit_at(parent, x, y)?, HTCAPTION as isize, "the strip drags the window there");
        for id in [crate::main_updates::LINK, crate::main_updates::DISMISS] {
            let (x, y) = centre(id)?;
            // SAFETY: Scalar lookup on this test's own window.
            let peer = unsafe { GetDlgItem(Some(parent), i32::from(id)) }?;
            assert_eq!(window_at(parent, x, y)?, peer, "peer {id} stays clickable");
        }
        Ok(())
    })();
    let shutdown = controls.shutdown();
    drop(controls);
    drop(presenter);
    drop(window);
    result?;
    shutdown?;
    Ok(())
}

#[test]
fn pressing_the_message_line_drags_the_window_while_its_buttons_stay_clickable() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::pressing_the_message_line_drags_the_window_while_its_buttons_stay_clickable"
        ),
        line_drag_body,
    )
}
