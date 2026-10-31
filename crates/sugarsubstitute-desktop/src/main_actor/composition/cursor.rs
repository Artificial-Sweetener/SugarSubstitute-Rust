// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present the composed browser's requested cursor over the host window's client area.
//!
//! A composition-hosted WebView2 owns no HWND, so Windows never asks it for a cursor. The host
//! selects the browser's cursor on client-area `WM_SETCURSOR` and immediately after
//! `CursorChanged` while the pointer is inside or captured, because Windows sends no
//! `WM_SETCURSOR` for a change without movement or while capture is held.

use super::*;
use std::cell::Cell;
use webview2_com::{
    CursorChangedEventHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2CompositionController2,
};
use windows::{
    Win32::{
        Foundation::{E_NOINTERFACE, LPARAM, WPARAM},
        UI::{
            Input::KeyboardAndMouse::GetCapture,
            WindowsAndMessaging::{HCURSOR, HTCLIENT, LoadCursorW, SetCursor},
        },
    },
    core::PCWSTR,
};

/// Pointer presence and the browser's latest resolved cursor, confined to the UI thread.
///
/// Shared system cursors are never destroyed; browser handles remain owned by WebView2.
#[derive(Default)]
pub(super) struct Pointer {
    inside: Cell<bool>,
    cursor: Cell<Option<HCURSOR>>,
}

impl Pointer {
    /// Record client-area entry; true only for the first move since the last leave.
    pub(super) fn enter(&self) -> bool {
        !self.inside.replace(true)
    }

    /// Record that the pointer left the client area or that leave tracking was not armed.
    pub(super) fn leave(&self) {
        self.inside.set(false);
    }

    /// The browser cursor for the next client-area `WM_SETCURSOR`, once the browser named one.
    pub(super) fn cursor(&self) -> Option<HCURSOR> {
        self.cursor.get()
    }

    /// Retain the browser's new cursor and return it when it must be selected immediately.
    pub(super) fn change(&self, cursor: Option<HCURSOR>, captured: bool) -> Option<HCURSOR> {
        self.cursor.set(cursor);
        cursor.filter(|_| self.inside.get() || captured)
    }
}

/// Claim `WM_SETCURSOR` only for this window's own client area.
/// Child caption controls and Tao's frame-edge hit tests keep their own cursors.
pub(super) fn claims(window: HWND, wparam: WPARAM, lparam: LPARAM) -> bool {
    wparam.0 == window.0 as usize && lparam.0 as u16 as u32 == HTCLIENT
}

/// `MAKEINTRESOURCE` accepts only nonzero 16-bit identifiers; zero reports a custom CSS cursor.
fn system_resource(id: u32) -> Option<u16> {
    u16::try_from(id).ok().filter(|id| *id != 0)
}

/// Prefer the DPI-aware shared system cursor; read the browser's own handle only otherwise.
/// A null browser handle leaves the window's ordinary cursor in place.
fn choose<E>(
    system: u32,
    load: impl FnOnce(u16) -> Option<HCURSOR>,
    handle: impl FnOnce() -> Result<HCURSOR, E>,
) -> Result<Option<HCURSOR>, E> {
    if let Some(cursor) = system_resource(system).and_then(load) {
        return Ok(Some(cursor));
    }
    let handle = handle()?;
    Ok((!handle.0.is_null()).then_some(handle))
}

/// Resolve the browser's current cursor through its retained composition controller.
/// Call on the controller's creating UI thread; native getter failures are preserved.
pub(super) fn resolve(
    controller: &ICoreWebView2CompositionController,
) -> Result<Option<HCURSOR>, DesktopError> {
    let mut system = 0;
    match controller.cast::<ICoreWebView2CompositionController2>() {
        // SAFETY: The retained UI-affine controller copies one scalar into live local storage.
        Ok(current) => unsafe { current.SystemCursorId(&raw mut system) }
            .map_err(|cause| DesktopError::new("observe_browser_system_cursor", cause))?,
        // Runtimes without SystemCursorId report every cursor through the handle getter.
        Err(cause) if cause.code() == E_NOINTERFACE => {}
        Err(cause) => return Err(DesktopError::new("observe_browser_system_cursor", cause)),
    }
    choose(system, load_system, || {
        let mut handle = HCURSOR::default();
        // SAFETY: The retained UI-affine controller copies one borrowed handle into local storage.
        unsafe { controller.Cursor(&raw mut handle) }
            .map(|()| handle)
            .map_err(|cause| DesktopError::new("observe_browser_cursor", cause))
    })
}

/// Load a shared system cursor; an identifier Windows does not know falls back to the handle.
fn load_system(id: u16) -> Option<HCURSOR> {
    // SAFETY: MAKEINTRESOURCE names a shared system cursor that callers never destroy.
    unsafe { LoadCursorW(None, PCWSTR(usize::from(id) as *const u16)) }.ok()
}

/// Select a browser cursor synchronously on the window's UI thread.
pub(super) fn select(cursor: HCURSOR) {
    // SAFETY: The handle is a shared system cursor or a live WebView2-owned browser cursor.
    unsafe {
        SetCursor(Some(cursor));
    }
}

/// Report whether this window holds pointer capture on the current UI thread.
pub(super) fn captured(window: HWND) -> bool {
    // SAFETY: Read-only observation of the calling UI thread's capture window.
    let capture = unsafe { GetCapture() };
    capture == window
}

/// Own one `CursorChanged` registration on the retained composition controller.
#[derive(Default)]
pub(super) struct Subscription {
    token: Option<i64>,
}

impl Subscription {
    /// Register once; a returned token is retained for removal even when the status fails.
    pub(super) fn install(
        &mut self,
        controller: &ICoreWebView2CompositionController,
        mut changed: impl FnMut() + 'static,
    ) -> Result<(), DesktopError> {
        if self.token.is_some() {
            return Ok(());
        }
        let handler = CursorChangedEventHandler::create(Box::new(move |_, _| {
            changed();
            Ok(())
        }));
        let mut token = 0;
        // SAFETY: The handler retains only UI-affine state; the token output is live local storage.
        let result = unsafe { controller.add_CursorChanged(&handler, &raw mut token) };
        if result.is_ok() || token != 0 {
            self.token = Some(token);
        }
        result.map_err(|cause| DesktopError::new("install_browser_cursor_events", cause))
    }

    /// Remove the registration before the controller is released; failure remains retryable.
    pub(super) fn retire(
        &mut self,
        controller: &ICoreWebView2CompositionController,
    ) -> Result<(), DesktopError> {
        if let Some(token) = self.token {
            // SAFETY: This exact token belongs to the retained controller on its UI thread.
            unsafe { controller.remove_CursorChanged(token) }
                .map_err(|cause| DesktopError::new("remove_browser_cursor_events", cause))?;
            self.token = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
