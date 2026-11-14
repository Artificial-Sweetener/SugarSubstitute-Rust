// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render the real Main window through the compositor on a private desktop, restored and
//! maximized, and check that the system draws no caption glyphs of its own: the Main caption's
//! buttons are transparent over the window's material, so any system glyph beneath them would
//! show as a second copy.
use super::tests::{Surface, TestResult, WIDTH};
use super::*;
use tao::{dpi::LogicalSize, event_loop::EventLoopBuilder, window::WindowBuilder};
use windows::Win32::{
    Graphics::{Dwm::DwmFlush, Gdi::HDC},
    UI::WindowsAndMessaging::GetClientRect,
};

windows::core::link!("user32.dll" "system" fn PrintWindow(hwnd: HWND, dc: HDC, flags: u32) -> windows::core::BOOL);

/// The window-button area at the client's top right as the compositor presents it: the count
/// of glyph-bright pixels and of unbacked (white) pixels.
fn button_area(parent: HWND, scale: f64) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    // SAFETY: Waits for the compositor to present this desktop's latest frame; no input occurs.
    unsafe { DwmFlush() }?;
    let mut client = windows::Win32::Foundation::RECT::default();
    // SAFETY: Read-only client bounds of this test's own window.
    unsafe { GetClientRect(parent, &raw mut client) }?;
    let surface = Surface::new(client.right, client.bottom)?;
    // PW_CLIENTONLY | PW_RENDERFULLCONTENT: the composed client, every compositor layer included.
    // SAFETY: Prints this test's own window into its owned memory bitmap.
    if !unsafe { PrintWindow(parent, surface.dc, 3) }.as_bool() {
        return Err("PrintWindow failed".into());
    }
    let width = (f64::from(crate::main_caption::CAPTION_WIDTH) * scale).ceil() as i32;
    let height = (f64::from(crate::main_caption::TITLEBAR_HEIGHT) * scale).ceil() as i32;
    let pixels = surface.pixels();
    let (mut glyph, mut unbacked) = (0, 0);
    for y in 0..height.min(client.bottom) {
        for x in (client.right - width).max(0)..client.right {
            let at = ((y * client.right + x) * 4) as usize;
            let bgr = &pixels[at..at + 3];
            if bgr.iter().all(|&value| value > 240) {
                unbacked += 1;
            } else if bgr.iter().all(|&value| value > 96) {
                glyph += 1;
            }
        }
    }
    Ok((glyph, unbacked))
}

/// A Main window without caption peers shows no window-button glyph, restored or maximized,
/// and its material still backs the maximized caption strip.
fn maximized_body() -> TestResult {
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    tao::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);
    let events = builder.build();
    // Built as the Main window is: frameless, resizable and transparent, over Mica Alt.
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("Main maximized frame qualification")
            .with_inner_size(LogicalSize::new(f64::from(WIDTH), 600.0))
            .with_decorations(false)
            .with_resizable(true)
            .with_transparent(true)
            .with_visible(true)
            .build(&events)?,
    );
    let parent = HWND(window.hwnd() as *mut c_void);
    let presenter = crate::windows::Presenter::new_main(window.clone(), true)?;
    let scale = window.scale_factor();
    let restored = button_area(parent, scale)?;
    window.set_maximized(true);
    let maximized = button_area(parent, scale)?;
    window.set_maximized(false);
    let restored_again = button_area(parent, scale)?;
    drop(presenter);
    drop(window);
    for (state, (glyph, _)) in
        [("restored", restored), ("maximized", maximized), ("restored again", restored_again)]
    {
        assert_eq!(glyph, 0, "{state} at scale {scale}: system caption glyph pixels");
    }
    // The client is unpainted here, so only the maximized frame shows the material beneath it.
    assert_eq!(maximized.1, 0, "maximized at scale {scale}: caption strip without its material");
    Ok(())
}

#[test]
fn the_system_draws_no_caption_buttons_under_the_main_caption() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::the_system_draws_no_caption_buttons_under_the_main_caption"),
        maximized_body,
    )
}
