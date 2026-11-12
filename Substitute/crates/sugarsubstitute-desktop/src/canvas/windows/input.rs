// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve hardware wheel units and native interaction lifetimes before Tao's scroll scaling.
use super::super::click::{Click, double_click};
use super::super::feedback::FeedbackTarget;
use super::super::latency::InputDelay;
use super::cursor::{CursorResource, Cursors, OwnedCursor};
use super::*;
use crate::windows_subclass::{self, Registration};
use ::windows::Win32::{
    Foundation::{LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    System::SystemInformation::GetTickCount,
    UI::{
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        Input::KeyboardAndMouse::{GetCapture, GetDoubleClickTime, ReleaseCapture, SetCapture},
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            GetClientRect, GetCursor, GetCursorPos, GetMessageTime, HTCLIENT, IsWindowVisible,
            SM_CXDOUBLECLK, SM_CYDOUBLECLK, SetCursor, WM_CANCELMODE, WM_CAPTURECHANGED,
            WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_KILLFOCUS,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY,
            WM_RBUTTONUP, WM_SETCURSOR, WM_SETTINGCHANGE, WM_SIZE, WM_SYSCOLORCHANGE,
            WM_THEMECHANGED, WM_XBUTTONUP, WindowFromPoint,
        },
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use sugarsubstitute_canvas::CursorIntent;
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
struct State {
    input_delay: Cell<InputDelay>,
    send: Box<dyn Fn(NativeEvent) -> Result<(), DesktopError>>,
    cursors: Cursors,
    failure: RefCell<Option<DesktopError>>,
    click: Cell<Option<Click>>,
    releasing: Cell<bool>,
    capture_owned: Cell<bool>,
    capture_lost: Cell<bool>,
    moving: Cell<bool>,
    cursor_intent: Cell<Option<sugarsubstitute_canvas::CursorIntent>>,
    cursor: RefCell<Option<CursorResource>>,
    selected_cursor: RefCell<Option<Rc<OwnedCursor>>>,
}
impl State {
    fn observe_input_age(&self) {
        // SAFETY: Both calls copy clocks on this message's owning thread without retaining handles.
        let (now, sent) = unsafe { (GetTickCount(), GetMessageTime()) };
        let mut delay = self.input_delay.get();
        delay.observe(now, sent);
        self.input_delay.set(delay);
    }
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
    /// Reconcile only canvas-owned capture; Windows retains its nonclient move/size capture.
    fn capture(&self, window: HWND, active: bool) -> Result<bool, DesktopError> {
        // SAFETY: This owner retains the original live HWND on the same presentation thread.
        let captured = unsafe { GetCapture() };
        if active {
            if self.moving.get()
                || self.capture_lost.get()
                || (!captured.is_invalid() && captured != window)
            {
                return Ok(false);
            }
            if captured.is_invalid() {
                // SAFETY: The requested owner is this same live window; no handle escapes.
                unsafe {
                    SetCapture(window);
                }
                // SAFETY: Query this UI thread's resulting capture without retaining a peer.
                if unsafe { GetCapture() } != window {
                    return Err(DesktopError::message(
                        "canvas_capture",
                        "Native pointer capture was rejected",
                    ));
                }
            }
            self.capture_owned.set(true);
        } else {
            if self.capture_owned.get() && captured == window {
                self.releasing.set(true);
                // SAFETY: Release only capture admitted to the canvas by this state owner.
                let result = unsafe { ReleaseCapture() };
                self.releasing.set(false);
                result.map_err(|cause| DesktopError::new("canvas_release_capture", cause))?;
            }
            self.capture_owned.set(false);
            self.capture_lost.set(false);
        }
        Ok(true)
    }
    /// Revoke logical canvas capture without releasing capture used by a native move loop.
    fn lose_capture(&self) {
        if self.capture_owned.replace(false) {
            self.capture_lost.set(true);
            self.send(NativeEvent::CaptureLost);
        }
    }
    fn pointer_owned(&self, window: HWND) -> Result<bool, DesktopError> {
        // SAFETY: Read the original peer's visibility without creating or focusing another peer.
        if !unsafe { IsWindowVisible(window) }.as_bool() {
            return Ok(false);
        }
        if self.capture_owned.get() {
            return Ok(true);
        }
        let mut point = POINT::default();
        // SAFETY: Stack storage remains live through this synchronous input-desktop query.
        unsafe { GetCursorPos(&mut point) }
            .map_err(|cause| DesktopError::new("canvas_cursor_position", cause))?;
        // SAFETY: These queries retain no peer and never change another control's input.
        if unsafe { WindowFromPoint(point) } != window {
            return Ok(false);
        }
        let mut client = point;
        let mut bounds = RECT::default();
        // SAFETY: Both writes target local storage for the same retained original HWND.
        let converted = unsafe { ScreenToClient(window, &mut client) }.as_bool();
        // SAFETY: The original HWND remains live; the complete client rect is copied out.
        unsafe { GetClientRect(window, &mut bounds) }
            .map_err(|cause| DesktopError::new("canvas_cursor_bounds", cause))?;
        Ok(converted
            && client.x >= bounds.left
            && client.x < bounds.right
            && client.y >= bounds.top
            && client.y < bounds.bottom)
    }
    fn select_cursor(&self, cursor: &CursorResource) {
        *self.selected_cursor.borrow_mut() = cursor.select();
    }
    fn publish_cursor(
        &self,
        window: HWND,
        intent: sugarsubstitute_canvas::CursorIntent,
    ) -> Result<(), Box<dyn Error>> {
        let cursor = self.cursors.get(window, intent)?;
        self.cursor_intent.set(Some(intent));
        if self.cursor.borrow().as_ref().is_some_and(|current| current.handle() == cursor.handle())
        {
            return Ok(());
        }
        *self.cursor.borrow_mut() = Some(cursor.clone());
        if self.pointer_owned(window)? {
            self.select_cursor(&cursor);
        }
        Ok(())
    }
    fn refresh_cursor(&self, window: HWND, force: bool) -> Result<(), Box<dyn Error>> {
        self.cursors.refresh(window, force)?;
        let Some(intent) = self.cursor_intent.get() else {
            return Ok(());
        };
        let cursor = self.cursors.get(window, intent)?;
        *self.cursor.borrow_mut() = Some(cursor.clone());
        if self.pointer_owned(window)? {
            self.select_cursor(&cursor);
        }
        Ok(())
    }
    fn retire_cursors(&self) {
        let selected = self.selected_cursor.borrow().as_ref().map(|cursor| cursor.handle());
        // SAFETY: Restore only when this state still owns the exact current custom cursor. A shared
        // arrow remains Windows-owned and prevents destroying an in-use private handle.
        if selected.is_some_and(|cursor| unsafe { GetCursor() } == cursor) {
            // SAFETY: The shared arrow remains OS-owned beyond this private cursor's lifetime.
            unsafe {
                SetCursor(Some(self.cursors.arrow()));
            }
        }
        self.selected_cursor.borrow_mut().take();
        self.cursor.borrow_mut().take();
        self.cursor_intent.set(None);
        self.cursors.retire_custom();
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
            input_delay: Cell::new(InputDelay::default()),
            send: Box::new(send),
            cursors,
            failure: RefCell::new(None),
            click: Cell::new(None),
            releasing: Cell::new(false),
            capture_owned: Cell::new(false),
            capture_lost: Cell::new(false),
            moving: Cell::new(false),
            cursor_intent: Cell::new(None),
            cursor: RefCell::new(None),
            selected_cursor: RefCell::new(None),
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
    /// Drain primitive input-age observations without logging or allocating in native callbacks.
    pub(super) fn take_input_delay(&self) -> InputDelay {
        self.state.input_delay.replace(InputDelay::default())
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
        self.state.capture(self.window, active)
    }
    /// Publish the requested cursor only while the canvas client or its gesture owns the pointer.
    pub(super) fn cursor(
        &self,
        intent: sugarsubstitute_canvas::CursorIntent,
    ) -> Result<(), Box<dyn Error>> {
        self.state.publish_cursor(self.window, intent)
    }
    /// Observe callback retirement, restore a selected private cursor and release its resources.
    pub(super) fn retire(&self) -> Result<(), Box<dyn Error>> {
        if !self.retired() {
            return Err("Native input callback has not retired".into());
        }
        self.state.retire_cursors();
        self.check().map_err(Box::<dyn Error>::from)
    }
}

impl FeedbackTarget for NativeInput {
    fn publish_cursor(&self, cursor: CursorIntent) -> Result<(), Box<dyn Error>> {
        self.cursor(cursor)
    }
    fn reconcile_capture(&self, capture: bool) -> Result<bool, Box<dyn Error>> {
        self.capture(capture).map_err(Box::<dyn Error>::from)
    }
}

fn point(value: LPARAM) -> [i32; 2] {
    [value.0 as u16 as i16 as i32, (value.0 >> 16) as u16 as i16 as i32]
}
/// Hardware notches precede Tao's OS scroll-line multiplier, including signed partial notches.
fn wheel_steps(value: WPARAM) -> f64 {
    f64::from((value.0 >> 16) as u16 as i16) / 120.0
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
    let refresh_cursor = match message {
        WM_SIZE => Some(false),
        WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE | WM_SYSCOLORCHANGE
        | WM_THEMECHANGED => Some(true),
        _ => None,
    };
    if matches!(message, WM_MOUSEMOVE | WM_MOUSEWHEEL) {
        state.observe_input_age();
    }
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
        WM_CAPTURECHANGED if lparam.0 != window.0 as isize => {
            // Tao releases its own button capture synchronously inside the forwarded button-up.
            // That ordinary release ends the gesture but leaves the first click eligible to pair.
            if !state.releasing.get() || lparam.0 != 0 {
                state.click.set(None);
                state.lose_capture();
            }
        }
        WM_ENTERSIZEMOVE => {
            state.moving.set(true);
            state.click.set(None);
            state.lose_capture();
        }
        WM_EXITSIZEMOVE => state.moving.set(false),
        WM_CANCELMODE | WM_KILLFOCUS => {
            state.click.set(None);
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => state.send(NativeEvent::DisplayChanged),
        WM_SETCURSOR if wparam.0 == window.0 as usize && lparam.0 as u16 as u32 == HTCLIENT => {
            if let Some(cursor) = state.cursor.borrow().clone() {
                state.select_cursor(&cursor);
                return LRESULT(1);
            }
        }
        WM_NCDESTROY => {
            state.retire_cursors();
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
    if let Some(force) = refresh_cursor
        && let Err(cause) = state.refresh_cursor(window, force)
    {
        state.fail(DesktopError::boxed("canvas_cursor_refresh", cause));
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
}
