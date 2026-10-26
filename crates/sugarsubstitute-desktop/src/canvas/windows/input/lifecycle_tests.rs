// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native input and callback-owned cursor custody without visible windows or Tao.
use super::*;
use ::windows::{
    Win32::{
        Graphics::Gdi::{ClientToScreen, DeleteObject, HGDIOBJ},
        UI::{
            Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture},
            WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, GetCursor, GetIconInfo, HICON, ICONINFO,
                SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CANCELMODE, WM_CAPTURECHANGED,
                WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_KILLFOCUS, WM_LBUTTONDOWN,
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
/// Observe this test thread's current capture without retaining the returned identity.
fn captured() -> HWND {
    // SAFETY: Read-only current-thread input state; no handle ownership is transferred.
    unsafe { GetCapture() }
}
#[test]
fn native_callback_keeps_shared_cursor_selection_after_owner_drop_until_final_destruction()
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
    Ok(())
}

#[test]
fn canvas_capture_never_releases_or_adopts_native_move_capture() -> Result<(), Box<dyn Error>> {
    let mut window = Window::new()?;
    let handle = window.handle()?;
    let input = NativeInput::new(handle, |_| Ok(()))?;
    // SAFETY: This test thread owns the hidden fixture and starts without another capture owner.
    unsafe {
        assert!(captured().is_invalid());
        SetCapture(handle);
    }
    assert_eq!(captured(), handle);
    assert!(input.state.capture(handle, false)?);
    assert_eq!(captured(), handle);
    assert!(!input.state.capture_owned.get());

    input.state.moving.set(true);
    assert!(!input.state.capture(handle, true)?);
    assert_eq!(captured(), handle);
    assert!(!input.state.capture_owned.get());
    input.state.moving.set(false);
    // SAFETY: Release exactly the simulated OS-owned capture before testing canvas acquisition.
    unsafe { ReleaseCapture() }?;

    assert!(input.state.capture(handle, true)?);
    assert_eq!(captured(), handle);
    assert!(input.state.capture_owned.get());
    assert!(input.state.capture(handle, false)?);
    assert!(captured().is_invalid());
    assert!(!input.state.capture_owned.get());
    window.destroy()?;
    input.retire()?;
    Ok(())
}

#[test]
fn owned_button_release_preserves_the_first_click_for_native_double_click_pairing()
-> Result<(), Box<dyn Error>> {
    let mut window = Window::new()?;
    let handle = window.handle()?;
    let double_clicks = Rc::new(Cell::new(0));
    let observed = double_clicks.clone();
    let input = NativeInput::new(handle, move |event| {
        if matches!(event, NativeEvent::DoubleClick(_)) {
            observed.set(observed.get() + 1);
        }
        Ok(())
    })?;
    let point = LPARAM((11_u32 << 16 | 7) as isize);
    // SAFETY: Messages and capture target only this same-thread hidden fixture.
    unsafe {
        SendMessageW(handle, WM_LBUTTONDOWN, None, Some(point));
    }
    assert!(input.state.click.get().is_some());
    assert!(input.state.capture(handle, true)?);
    assert!(input.state.capture(handle, false)?);
    assert!(input.state.click.get().is_some());
    // SAFETY: The second down is synchronous and cannot activate the hidden styleless peer.
    unsafe {
        SendMessageW(handle, WM_LBUTTONDOWN, None, Some(point));
    }
    assert_eq!(double_clicks.get(), 1);
    assert!(input.state.click.get().is_none());
    window.destroy()?;
    input.retire()?;
    Ok(())
}

#[test]
fn external_capture_focus_cancel_and_native_move_clear_click_pairing() -> Result<(), Box<dyn Error>>
{
    let mut window = Window::new()?;
    let handle = window.handle()?;
    let losses = Rc::new(Cell::new(0));
    let observed = losses.clone();
    let input = NativeInput::new(handle, move |event| {
        if matches!(event, NativeEvent::CaptureLost) {
            observed.set(observed.get() + 1);
        }
        Ok(())
    })?;
    let click = Click { time: 1, point: [7, 11] };
    input.state.click.set(Some(click));
    input.state.capture_owned.set(true);
    // SAFETY: Deliver only cancellation notifications to this retained hidden fixture.
    unsafe {
        SendMessageW(handle, WM_CAPTURECHANGED, None, Some(LPARAM(0)));
    }
    assert!(input.state.click.get().is_none());
    assert_eq!(losses.get(), 1);

    input.state.click.set(Some(click));
    input.state.capture_owned.set(true);
    input.state.releasing.set(true);
    // SAFETY: A distinct nonzero gaining owner models capture theft during button-up reentry.
    unsafe {
        SendMessageW(handle, WM_CAPTURECHANGED, None, Some(LPARAM(1)));
    }
    input.state.releasing.set(false);
    assert!(input.state.click.get().is_none());
    assert_eq!(losses.get(), 2);

    for message in [WM_CANCELMODE, WM_KILLFOCUS, WM_ENTERSIZEMOVE] {
        input.state.click.set(Some(click));
        // SAFETY: Each synchronous message targets only the private hidden peer.
        unsafe {
            SendMessageW(handle, message, None, None);
        }
        assert!(input.state.click.get().is_none());
    }
    // SAFETY: Balance the synthetic native move lifecycle before destroying the peer.
    unsafe {
        SendMessageW(handle, WM_EXITSIZEMOVE, None, None);
    }
    window.destroy()?;
    input.retire()?;
    Ok(())
}

#[test]
fn native_move_messages_revoke_only_canvas_capture_once() -> Result<(), Box<dyn Error>> {
    let mut window = Window::new()?;
    let losses = Rc::new(Cell::new(0));
    let observed = losses.clone();
    let input = NativeInput::new(window.handle()?, move |event| {
        if matches!(event, NativeEvent::CaptureLost) {
            observed.set(observed.get() + 1);
        }
        Ok(())
    })?;
    input.state.capture_owned.set(true);
    // SAFETY: Synchronously deliver lifecycle notifications to this test's hidden original HWND.
    unsafe {
        SendMessageW(window.handle()?, WM_ENTERSIZEMOVE, None, None);
    }
    assert!(input.state.moving.get());
    assert!(!input.state.capture_owned.get());
    assert_eq!(losses.get(), 1);
    // An OS-owned capture transition must not manufacture another canvas loss.
    // SAFETY: This test retains the original hidden HWND on its native thread through both calls.
    unsafe {
        SendMessageW(window.handle()?, WM_CAPTURECHANGED, None, Some(LPARAM(0)));
        SendMessageW(window.handle()?, WM_EXITSIZEMOVE, None, None);
    }
    assert!(!input.state.moving.get());
    assert_eq!(losses.get(), 1);
    window.destroy()?;
    input.retire()?;
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
