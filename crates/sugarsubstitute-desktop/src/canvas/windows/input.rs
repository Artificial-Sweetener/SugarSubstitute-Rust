// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve hardware wheel units and native interaction lifetimes before Tao's scroll scaling.
use super::cursor::Cursors;
use super::*;
use crate::windows_subclass::{self, Registration};
use ::windows::Win32::{
    Foundation::{LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        Input::KeyboardAndMouse::{GetCapture, GetDoubleClickTime, ReleaseCapture, SetCapture},
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            GetCursorPos, GetMessageTime, HCURSOR, HTCLIENT, IsWindowVisible, SM_CXDOUBLECLK,
            SM_CYDOUBLECLK, SetCursor, WM_CANCELMODE, WM_CAPTURECHANGED, WM_DISPLAYCHANGE,
            WM_DPICHANGED, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONUP, WM_MOUSEWHEEL,
            WM_NCDESTROY, WM_RBUTTONUP, WM_SETCURSOR, WM_XBUTTONUP, WindowFromPoint,
        },
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[cfg(test)]
mod lifecycle_tests;

/// Native input uses the same event loop as execution completion notification.
#[derive(Clone, Copy, Debug)]
pub(super) enum NativeEvent {
    Wake,
    Wheel { point: [f64; 2], steps: f64 },
    DoubleClick([f64; 2]),
    CaptureLost,
    DisplayChanged,
}
#[derive(Clone, Copy)]
struct Click {
    time: u32,
    point: [i32; 2],
}
struct State {
    send: Box<dyn Fn(NativeEvent) -> Result<(), DesktopError>>,
    cursors: RefCell<Option<Cursors>>,
    failure: RefCell<Option<DesktopError>>,
    click: Cell<Option<Click>>,
    releasing: Cell<bool>,
    capture_lost: Cell<bool>,
    cursor: Cell<Option<HCURSOR>>,
}
impl State {
    fn send(&self, event: NativeEvent) {
        if let Err(cause) = (self.send)(event) {
            self.fail(cause);
        }
    }
    fn fail(&self, cause: DesktopError) {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() {
            *failure = Some(cause);
        }
    }
}
/// Retain callback data until the original HWND's final destruction is observed.
pub(super) struct NativeInput {
    state: Rc<State>,
    registration: Registration<Rc<State>>,
    window: HWND,
}
impl NativeInput {
    pub(super) fn new(
        window: HWND,
        send: impl Fn(NativeEvent) -> Result<(), DesktopError> + 'static,
    ) -> Result<Self, Box<dyn Error>> {
        let cursors = Cursors::new(window)?;
        let state = Rc::new(State {
            send: Box::new(send),
            cursors: RefCell::new(Some(cursors)),
            failure: RefCell::new(None),
            click: Cell::new(None),
            releasing: Cell::new(false),
            capture_lost: Cell::new(false),
            cursor: Cell::new(None),
        });
        let callback = Rc::new(state.clone());
        let registration = Registration::new(&callback);
        // SAFETY: The desktop owns this live HWND and retains the matching concrete registration
        // on its UI thread through final WM_NCDESTROY, which returns the native reference once.
        unsafe {
            windows_subclass::install(window, Some(procedure), &registration)?;
        }
        Ok(Self { state, registration, window })
    }
    pub(super) fn check(&self) -> Result<(), DesktopError> {
        self.state.failure.borrow_mut().take().map_or(Ok(()), Err)
    }
    pub(super) fn retired(&self) -> bool {
        self.registration.destroyed()
    }
    /// Reconcile visible-peer capture without taking another control's input.
    /// Hidden presentation peers retain their logical gestures without acquiring native capture.
    pub(super) fn capture(&self, active: bool) -> Result<bool, DesktopError> {
        // SAFETY: This owner retains the original HWND on the same native UI thread.
        if !unsafe { IsWindowVisible(self.window) }.as_bool() {
            return Ok(true);
        }
        // SAFETY: This owner retains the original live HWND on the same presentation thread.
        let captured = unsafe { GetCapture() };
        if active && captured != self.window {
            if !captured.is_invalid() || self.state.capture_lost.get() {
                return Ok(false);
            }
            // SAFETY: The requested owner is this same live window; no handle escapes.
            unsafe {
                SetCapture(self.window);
            }
            // SAFETY: Query this UI thread's resulting capture without retaining another peer.
            if unsafe { GetCapture() } != self.window {
                return Err(DesktopError::message(
                    "canvas_capture",
                    "Native pointer capture was rejected",
                ));
            }
        } else if !active && captured == self.window {
            self.state.releasing.set(true);
            // SAFETY: Only this window's current capture is released. The flag distinguishes
            // the synchronous WM_CAPTURECHANGED notification from unexpected capture loss.
            let result = unsafe { ReleaseCapture() };
            self.state.releasing.set(false);
            result.map_err(|cause| DesktopError::new("canvas_release_capture", cause))?;
        }
        if !active {
            self.state.capture_lost.set(false);
        }
        Ok(true)
    }
    /// Publish a cursor only after fallible preparation succeeds; callback state retains its owner.
    pub(super) fn cursor(
        &self,
        intent: sugarsubstitute_canvas::CursorIntent,
    ) -> Result<(), Box<dyn Error>> {
        let replace = self
            .state
            .cursors
            .borrow()
            .as_ref()
            .ok_or("Native cursor resources have retired")?
            .changed(self.window);
        let replacement = if replace { Some(Cursors::new(self.window)?) } else { None };
        let cursor = if let Some(cursors) = &replacement {
            cursors.get(intent)?
        } else {
            self.state
                .cursors
                .borrow()
                .as_ref()
                .ok_or("Native cursor resources have retired")?
                .get(intent)?
        };
        if self.state.cursor.get() == Some(cursor) {
            return Ok(());
        }
        // SAFETY: Read the original peer's visibility without creating or focusing another peer.
        let visible = unsafe { IsWindowVisible(self.window) }.as_bool();
        let mut owned = false;
        if visible {
            let mut point = POINT::default();
            // SAFETY: Stack storage remains live through this synchronous input-desktop query.
            unsafe { GetCursorPos(&mut point) }
                .map_err(|cause| DesktopError::new("canvas_cursor_position", cause))?;
            // SAFETY: These queries retain no peer and never change another control's input.
            owned = unsafe { WindowFromPoint(point) == self.window || GetCapture() == self.window };
        }
        let retired =
            replacement.and_then(|cursors| self.state.cursors.borrow_mut().replace(cursors));
        self.state.cursor.set(Some(cursor));
        if owned {
            // SAFETY: Callback state owns this cursor through final HWND retirement.
            unsafe {
                SetCursor(Some(cursor));
            }
        }
        if let Some(mut retired) = retired {
            retired.retire()?;
        }
        Ok(())
    }
    /// Release native resources after the actual final callback, preserving delivery and close errors.
    pub(super) fn retire(&self) -> Result<(), Box<dyn Error>> {
        if !self.retired() {
            return Err("Native input callback has not retired".into());
        }
        let observed = self.check().map_err(Box::<dyn Error>::from);
        let retired = self
            .state
            .cursors
            .borrow_mut()
            .take()
            .map_or(Ok(()), |mut cursors| cursors.retire())
            .map_err(Box::<dyn Error>::from);
        self.state.cursor.set(None);
        combine(observed, retired)
    }
}

fn point(value: LPARAM) -> [i32; 2] {
    [value.0 as u16 as i16 as i32, (value.0 >> 16) as u16 as i16 as i32]
}
/// Hardware notches precede Tao's OS scroll-line multiplier, including signed partial notches.
fn wheel_steps(value: WPARAM) -> f64 {
    f64::from((value.0 >> 16) as u16 as i16) / 120.0
}
/// Match physical double-click bounds and the native wrapping 32-bit message timestamp.
fn double_click(previous: Click, current: Click, bounds: [i32; 2], limit: u32) -> bool {
    bounds.iter().all(|bound| *bound > 0)
        && current.time.wrapping_sub(previous.time) <= limit
        && (0..2).all(|axis| {
            (i64::from(current.point[axis]) - i64::from(previous.point[axis])).abs() * 2
                <= i64::from(bounds[axis])
        })
}
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let state = {
        // SAFETY: Installation supplies this exact registered concrete type. Clone stable
        // custody before forwarding; no borrow of registration memory crosses native reentry.
        unsafe { windows_subclass::callback::<Rc<State>>(data) }.clone()
    };
    match message {
        WM_MOUSEWHEEL => {
            let position = point(lparam);
            let mut position = POINT { x: position[0], y: position[1] };
            // SAFETY: The live client peer belongs to this callback; POINT storage is local.
            if unsafe { ScreenToClient(window, &mut position) }.as_bool() {
                state.send(NativeEvent::Wheel {
                    point: [f64::from(position.x), f64::from(position.y)],
                    steps: wheel_steps(wparam),
                });
            } else {
                state.fail(DesktopError::message(
                    "canvas_wheel",
                    "Native wheel coordinates are unavailable",
                ));
            }
            return LRESULT(0);
        }
        WM_LBUTTONDOWN => {
            let position = point(lparam);
            // SAFETY: Read this native UI thread's message timestamp and this live peer's DPI.
            let time = unsafe { GetMessageTime() } as u32;
            // SAFETY: This callback's HWND is still live on the owning UI thread.
            let dpi = unsafe { GetDpiForWindow(window) };
            // SAFETY: The metric queries copy values for that same native physical DPI.
            let bounds = unsafe {
                [
                    GetSystemMetricsForDpi(SM_CXDOUBLECLK, dpi),
                    GetSystemMetricsForDpi(SM_CYDOUBLECLK, dpi),
                ]
            };
            let matches = state.click.get().is_some_and(|click| {
                // SAFETY: Read the current OS double-click preference without changing it.
                double_click(click, Click { time, point: position }, bounds, unsafe {
                    GetDoubleClickTime()
                })
            });
            state.click.set(if matches { None } else { Some(Click { time, point: position }) });
            if matches {
                state.send(NativeEvent::DoubleClick(position.map(f64::from)));
            }
        }
        WM_CAPTURECHANGED if lparam.0 != window.0 as isize && !state.releasing.get() => {
            state.capture_lost.set(true);
            state.click.set(None);
            state.send(NativeEvent::CaptureLost);
        }
        WM_CANCELMODE | WM_KILLFOCUS => {
            state.click.set(None);
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => state.send(NativeEvent::DisplayChanged),
        WM_SETCURSOR if wparam.0 == window.0 as usize && lparam.0 as u16 as u32 == HTCLIENT => {
            if let Some(cursor) = state.cursor.get() {
                // SAFETY: NativeInput's cursor owner outlives the registered callback.
                unsafe {
                    SetCursor(Some(cursor));
                }
                return LRESULT(1);
            }
        }
        WM_NCDESTROY => {
            // SAFETY: This is the exact matching final callback; the shared owner remains live
            // across chaining and the registration reclaims its one native Rc reference once.
            return unsafe {
                windows_subclass::destroy_callback::<Rc<State>>(
                    window,
                    Some(procedure),
                    wparam,
                    lparam,
                    data,
                )
            };
        }
        _ => {}
    }
    let releasing = matches!(message, WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP | WM_XBUTTONUP);
    let previous = state.releasing.get();
    if releasing {
        state.releasing.set(true);
    }
    // SAFETY: Preserve Tao's complete native window, button and capture processing.
    let result = unsafe { DefSubclassProc(window, message, wparam, lparam) };
    if releasing {
        state.releasing.set(previous);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_wheel_units_preserve_signed_partial_and_multiple_hardware_notches() {
        for (native, expected) in [
            (-360_i16, -3.0),
            (-120, -1.0),
            (-60, -0.5),
            (0, 0.0),
            (30, 0.25),
            (120, 1.0),
            (360, 3.0),
        ] {
            assert_eq!(wheel_steps(WPARAM((usize::from(native as u16) << 16) | 7)), expected);
        }
        assert_eq!(point(LPARAM(0xfffd_fffe_u32 as isize)), [-2, -3]);
    }
    #[test]
    fn double_click_matches_native_time_and_physical_distance_including_timestamp_wrap() {
        let previous = Click { time: u32::MAX - 50, point: [20, -30] };
        assert!(double_click(previous, Click { time: 25, point: [22, -28] }, [4, 4], 100));
        assert!(!double_click(previous, Click { time: 25, point: [23, -28] }, [4, 4], 100));
        assert!(!double_click(previous, Click { time: 50, point: [20, -30] }, [4, 4], 100));
        assert!(!double_click(previous, Click { time: 25, point: [20, -30] }, [0, 0], 100));
    }
}
