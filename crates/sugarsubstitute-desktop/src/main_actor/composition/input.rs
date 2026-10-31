// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Host the composed browser's native pointer: mouse messages, cursor and drag-and-drop.

use super::callbacks::{CallbackFence, retire_when_quiet};
use super::cursor::{self, Pointer, Subscription};
use super::drop_target::DropTarget;
use super::*;
use crate::windows_subclass::{self, Registration};
use std::cell::Cell;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN,
            WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY,
            WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_XBUTTONDBLCLK,
            WM_XBUTTONDOWN, WM_XBUTTONUP,
        },
    },
};

/// WebView2 accepts the seven Win32 mouse-button, shift and control key bits.
const MOUSE_KEYS: u16 = 0x7f;

/// These values remain borrowed from the original native message until synchronous delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Mouse {
    kind: u32,
    keys: u16,
    data: u32,
    point: [i32; 2],
    screen: bool,
}

/// Decode only messages understood by WebView2; other window and caption messages stay with Tao.
fn mouse(message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<Mouse> {
    if message == WM_MOUSELEAVE {
        return Some(Mouse { kind: message, keys: 0, data: 0, point: [0, 0], screen: false });
    }
    if !matches!(
        message,
        WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_LBUTTONDBLCLK
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_RBUTTONDBLCLK
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_MBUTTONDBLCLK
            | WM_XBUTTONDOWN
            | WM_XBUTTONUP
            | WM_XBUTTONDBLCLK
            | WM_MOUSEWHEEL
            | WM_MOUSEHWHEEL
    ) {
        return None;
    }
    let keys = wparam.0 as u16;
    if keys & !MOUSE_KEYS != 0 {
        return None;
    }
    let screen = matches!(message, WM_MOUSEWHEEL | WM_MOUSEHWHEEL);
    let high = (wparam.0 >> 16) as u16;
    if matches!(message, WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK) && !matches!(high, 1 | 2)
    {
        return None;
    }
    let data = if screen {
        (high as i16 as i32) as u32
    } else if matches!(message, WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK) {
        u32::from(high)
    } else {
        0
    };
    Some(Mouse {
        kind: message,
        keys,
        data,
        point: [lparam.0 as u16 as i16 as i32, (lparam.0 >> 16) as u16 as i16 as i32],
        screen,
    })
}

struct State {
    window: HWND,
    controller: ICoreWebView2CompositionController,
    events: Rc<Events>,
    active: Cell<bool>,
    pointer: Pointer,
    callbacks: CallbackFence,
}

impl State {
    /// Deliver in physical client pixels on the original UI apartment, without retaining a borrow.
    fn deliver(&self, message: Mouse) -> Result<(), DesktopError> {
        if message.kind == WM_MOUSEMOVE && self.pointer.enter() {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: self.window,
                dwHoverTime: 0,
            };
            // SAFETY: The callback owns this live HWND and the native call borrows local storage.
            if let Err(cause) = unsafe { TrackMouseEvent(&raw mut tracking) } {
                self.pointer.leave();
                return Err(DesktopError::new("track_composed_browser_mouse", cause));
            }
        } else if message.kind == WM_MOUSELEAVE {
            self.pointer.leave();
        }
        let mut point = POINT { x: message.point[0], y: message.point[1] };
        if message.screen {
            // SAFETY: The original live HWND and local POINT belong to this UI-thread callback.
            if !unsafe { ScreenToClient(self.window, &raw mut point) }.as_bool() {
                return Err(DesktopError::message(
                    "composed_browser_mouse_coordinates",
                    "Native wheel coordinates are unavailable",
                ));
            }
        }
        // SAFETY: The controller is retained on its creating UI apartment until subclass removal.
        unsafe {
            self.controller.SendMouseInput(
                COREWEBVIEW2_MOUSE_EVENT_KIND(message.kind as i32),
                COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(i32::from(message.keys)),
                message.data,
                point,
            )
        }
        .map_err(|cause| DesktopError::new("send_composed_browser_mouse", cause))
    }

    fn admits(&self) -> bool {
        self.active.get() && self.events.accepting()
    }

    /// Resolve the browser's new cursor and select it when Windows will not ask for it.
    fn cursor_changed(&self) {
        if !self.admits() {
            return;
        }
        let _callback = self.events.callback();
        let _scope = match self.callbacks.enter(&self.events) {
            Ok(scope) => scope,
            Err(cause) => return self.events.fail(cause),
        };
        match cursor::resolve(&self.controller) {
            Ok(resolved) => {
                if let Some(now) = self.pointer.change(resolved, cursor::captured(self.window)) {
                    cursor::select(now);
                }
            }
            Err(cause) => self.events.fail(cause),
        }
    }
}

/// Retain the exact subclass, cursor subscription and drop registration until confirmed
/// removal or final HWND destruction; each is removed before the controller is released.
pub(super) struct Input {
    state: Rc<State>,
    registration: Registration<Rc<State>>,
    cursor: Subscription,
    drop_target: Option<DropTarget>,
}

impl Input {
    /// Install only on the owning main HWND, after its composition controller is ready.
    pub(super) fn new(
        window: HWND,
        controller: ICoreWebView2CompositionController,
        events: Rc<Events>,
    ) -> Result<Self, DesktopError> {
        let state = Rc::new(State {
            window,
            controller,
            events,
            active: Cell::new(true),
            pointer: Pointer::default(),
            callbacks: CallbackFence::default(),
        });
        let callback = Rc::new(state.clone());
        let registration = Registration::new(&callback);
        // SAFETY: This apartment owns the live HWND and retains the exact callback registration.
        unsafe { windows_subclass::install(window, Some(procedure), &registration)? };
        Ok(Self { state, registration, cursor: Subscription::default(), drop_target: None })
    }

    /// Attach cursor presentation and drag forwarding; a partial attachment remains retirable.
    pub(super) fn attach(&mut self) -> Result<(), DesktopError> {
        let observed = self.state.clone();
        self.cursor.install(&self.state.controller, move || observed.cursor_changed())?;
        if self.drop_target.is_none() {
            self.drop_target = Some(DropTarget::register(
                self.state.window,
                &self.state.controller,
                self.state.events.clone(),
            )?);
        }
        Ok(())
    }

    /// Fence further browser input before any asynchronous main-window retirement.
    pub(super) fn suspend(&self) {
        self.state.active.set(false);
        if let Some(drop_target) = &self.drop_target {
            drop_target.suspend();
        }
    }

    /// Revoke drag forwarding, remove the cursor subscription, then the subclass.
    /// Every failure retains its native custody for a later retry.
    pub(super) fn retire(&mut self) -> Result<bool, DesktopError> {
        self.suspend();
        if let Some(drop_target) = &mut self.drop_target {
            if !drop_target.retire()? {
                return Ok(false);
            }
            self.drop_target = None;
        }
        self.cursor.retire(&self.state.controller)?;
        retire_when_quiet(&self.state.callbacks, || {
            windows_subclass::remove(self.state.window, Some(procedure), &self.registration)
        })
    }
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: Installation retains this exact callback type for the complete native registration.
    let state = unsafe { windows_subclass::callback::<Rc<State>>(data) }.clone();
    if message == WM_NCDESTROY {
        state.active.set(false);
        // SAFETY: The final callback returns this exact native registration after chaining.
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
    if message == WM_SETCURSOR
        && state.admits()
        && cursor::claims(window, wparam, lparam)
        && let Some(current) = state.pointer.cursor()
    {
        cursor::select(current);
        return LRESULT(1);
    }
    if state.admits()
        && let Some(message) = mouse(message, wparam, lparam)
    {
        let _callback = state.events.callback();
        match state.callbacks.enter(&state.events) {
            Ok(_input) => {
                if let Err(cause) = state.deliver(message) {
                    state.events.fail(cause);
                }
            }
            Err(cause) => state.events.fail(cause),
        }
    }
    // SAFETY: Preserve Tao's own capture, focus, window and native caption processing.
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

#[cfg(test)]
mod tests;
