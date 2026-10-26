// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native input and callback-owned cursor custody without visible windows or Tao.
use super::*;
use ::windows::{
    Win32::{
        Graphics::Gdi::{ClientToScreen, DeleteObject, HGDIOBJ},
        UI::{
            Input::KeyboardAndMouse::GetCapture,
            WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, GetCursor, GetIconInfo, HICON, ICONINFO,
                SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE,
            },
        },
    },
    core::w,
};
use sugarsubstitute_canvas::CursorIntent;

/// Retain only this test thread's explicitly hidden native peer through final callback observation.
struct Window(Option<HWND>);
impl Window {
    fn new() -> Result<Self, Box<dyn Error>> {
        // SAFETY: STATIC is an OS class; the owned top-level peer has no visible or activation style.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                64,
                64,
                None,
                None,
                None,
                None,
            )?
        };
        Ok(Self(Some(window)))
    }
    fn handle(&self) -> Result<HWND, Box<dyn Error>> {
        self.0.ok_or_else(|| "Native fixture retired".into())
    }
    fn destroy(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(window) = self.0 {
            // SAFETY: This exact private HWND was created on this same test thread.
            unsafe {
                DestroyWindow(window)?;
            }
            self.0 = None;
        }
        Ok(())
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        if let Err(cause) = self.destroy() {
            tracing::error!(operation="canvas_input_fixture_retirement", cause=%cause,
                "Hidden native input fixture retirement failed");
        }
    }
}
#[test]
fn native_callback_keeps_cursor_custody_after_owner_drop_until_final_destruction()
-> Result<(), Box<dyn Error>> {
    let mut window = Window::new()?;
    let input = NativeInput::new(window.handle()?, |_| Ok(()))?;
    input.cursor(CursorIntent::OpenHand)?;
    let state = Rc::downgrade(&input.state);
    let cursor = input.state.cursor.get().ok_or("Cursor was not published")?;
    drop(input);
    assert!(state.strong_count() > 0);
    let mut info = ICONINFO::default();
    // SAFETY: The native subclass retains the cursor owner despite dropping the application handle.
    unsafe {
        GetIconInfo(HICON(cursor.0), &mut info)?;
    }
    // SAFETY: Both handles are private copies from GetIconInfo, not cursor-owned bitmap storage.
    let mask = unsafe { DeleteObject(HGDIOBJ(info.hbmMask.0)) }.as_bool();
    let color = if info.hbmColor.is_invalid() {
        true
    } else {
        // SAFETY: A non-null color handle is the second copy from that same query.
        unsafe { DeleteObject(HGDIOBJ(info.hbmColor.0)) }.as_bool()
    };
    assert!(mask && color);
    window.destroy()?;
    assert_eq!(state.strong_count(), 0);
    Ok(())
}
#[test]
fn hidden_peers_preserve_input_desktop_and_retire_only_after_the_real_final_callback()
-> Result<(), Box<dyn Error>> {
    let mut window = Window::new()?;
    let input = NativeInput::new(window.handle()?, |_| Ok(()))?;
    // SAFETY: Query this test thread's capture and current cursor without changing either.
    let before = unsafe { (GetCapture(), GetCursor()) };
    assert!(input.capture(true)?);
    input.cursor(CursorIntent::ClosedHand)?;
    // SAFETY: The same read-only queries verify the hidden fixture changed no input ownership.
    assert_eq!(unsafe { (GetCapture(), GetCursor()) }, before);
    assert!(input.retire().is_err());
    window.destroy()?;
    assert!(input.retired());
    input.retire()?;
    assert!(input.state.cursors.borrow().is_none());
    Ok(())
}
#[test]
fn native_wheel_observations_reach_the_typed_sink_without_a_toolkit() -> Result<(), Box<dyn Error>>
{
    let mut window = Window::new()?;
    let observations = Rc::new(RefCell::new(Vec::with_capacity(3)));
    let sink = observations.clone();
    let input = NativeInput::new(window.handle()?, move |event| {
        if let NativeEvent::Wheel { point, steps } = event {
            sink.borrow_mut().push((point, steps));
        }
        Ok(())
    })?;
    let mut screen = POINT { x: 7, y: 11 };
    // SAFETY: Convert a local test coordinate through its actual retained private native peer.
    unsafe { ClientToScreen(window.handle()?, &mut screen) }.ok()?;
    let packed = ((u32::from(screen.y as u16) << 16) | u32::from(screen.x as u16)) as isize;
    for delta in [120_i16, -60, 360] {
        // SAFETY: Send only to this test's hidden HWND; the subclass handles wheel messages before
        // the OS class and never changes focus, capture, pointer position or scroll preferences.
        unsafe {
            SendMessageW(
                window.handle()?,
                WM_MOUSEWHEEL,
                Some(WPARAM(usize::from(delta as u16) << 16)),
                Some(LPARAM(packed)),
            );
        }
    }
    input.check()?;
    assert_eq!(
        &*observations.borrow(),
        &[([7.0, 11.0], 1.0), ([7.0, 11.0], -0.5), ([7.0, 11.0], 3.0)]
    );
    window.destroy()?;
    input.retire()?;
    Ok(())
}
