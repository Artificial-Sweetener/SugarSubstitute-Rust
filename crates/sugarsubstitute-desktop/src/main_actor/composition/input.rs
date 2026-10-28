// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Forward the qualification window's native mouse messages to its composed browser.

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
            WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDBLCLK, WM_XBUTTONDOWN,
            WM_XBUTTONUP,
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
    controller: ICoreWebView2CompositionController,
    events: Rc<Events>,
    active: Cell<bool>,
    tracking: Cell<bool>,
    callbacks: CallbackFence,
}

/// Retain a removal wake only while a reentrant input callback actually owns the controller.
#[derive(Default)]
struct CallbackFence {
    active: Cell<usize>,
    waiting: Cell<bool>,
}

impl CallbackFence {
    /// Count the exact in-flight input operation before calling into WebView2.
    fn enter<'a>(&'a self, events: &'a Events) -> Result<InputCallback<'a>, DesktopError> {
        let active = self.active.get().checked_add(1).ok_or_else(|| {
            DesktopError::message("composed_browser_input", "native callback depth exhausted")
        })?;
        self.active.set(active);
        Ok(InputCallback { fence: self, events })
    }
}

/// Return one callback's custody and wake a deferred removal after the last return.
struct InputCallback<'a> {
    fence: &'a CallbackFence,
    events: &'a Events,
}

impl Drop for InputCallback<'_> {
    fn drop(&mut self) {
        let active = self.fence.active.get() - 1;
        self.fence.active.set(active);
        if active == 0 && self.fence.waiting.replace(false) {
            self.events.wake();
        }
    }
}

impl State {
    /// Deliver in physical client pixels on the original UI apartment, without retaining a borrow.
    fn deliver(&self, window: HWND, message: Mouse) -> Result<(), DesktopError> {
        if message.kind == WM_MOUSEMOVE && !self.tracking.replace(true) {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: 0,
            };
            // SAFETY: The callback owns this live HWND and the native call borrows local storage.
            if let Err(cause) = unsafe { TrackMouseEvent(&raw mut tracking) } {
                self.tracking.set(false);
                return Err(DesktopError::new("track_composed_browser_mouse", cause));
            }
        } else if message.kind == WM_MOUSELEAVE {
            self.tracking.set(false);
        }
        let mut point = POINT { x: message.point[0], y: message.point[1] };
        if message.screen {
            // SAFETY: The original live HWND and local POINT belong to this UI-thread callback.
            if !unsafe { ScreenToClient(window, &raw mut point) }.as_bool() {
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
}

/// Retain the exact subclass and controller until confirmed removal or final HWND destruction.
pub(super) struct Input {
    window: HWND,
    state: Rc<State>,
    registration: Registration<Rc<State>>,
}

impl Input {
    /// Install only on the owning main HWND, after its composition controller is ready.
    pub(super) fn new(
        window: HWND,
        controller: ICoreWebView2CompositionController,
        events: Rc<Events>,
    ) -> Result<Self, DesktopError> {
        let state = Rc::new(State {
            controller,
            events,
            active: Cell::new(true),
            tracking: Cell::new(false),
            callbacks: CallbackFence::default(),
        });
        let callback = Rc::new(state.clone());
        let registration = Registration::new(&callback);
        // SAFETY: This apartment owns the live HWND and retains the exact callback registration.
        unsafe { windows_subclass::install(window, Some(procedure), &registration)? };
        Ok(Self { window, state, registration })
    }

    /// Fence further browser input before any asynchronous main-window retirement.
    pub(super) fn suspend(&self) {
        self.state.active.set(false);
    }

    /// Remove the callback before detaching the controller; failure retains its native custody.
    pub(super) fn retire(&self) -> Result<bool, DesktopError> {
        self.suspend();
        retire_when_quiet(&self.state.callbacks, || {
            windows_subclass::remove(self.window, Some(procedure), &self.registration)
        })
    }
}

/// Do not detach the controller while an input callback is still using it.
fn retire_when_quiet(
    callbacks: &CallbackFence,
    remove: impl FnOnce() -> Result<(), DesktopError>,
) -> Result<bool, DesktopError> {
    if callbacks.active.get() != 0 {
        callbacks.waiting.set(true);
        return Ok(false);
    }
    remove()?;
    Ok(true)
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
    if state.active.get()
        && state.events.accepting()
        && let Some(message) = mouse(message, wparam, lparam)
    {
        let _callback = state.events.callback();
        match state.callbacks.enter(&state.events) {
            Ok(_input) => {
                if let Err(cause) = state.deliver(window, message) {
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
