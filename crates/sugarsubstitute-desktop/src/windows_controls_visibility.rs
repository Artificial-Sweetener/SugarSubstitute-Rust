// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply prepared-control availability to the original retained HWND and callback mapping.
use super::state::VisibilityPort;
use super::*;
use windows::Win32::{
    Foundation::{ERROR_SUCCESS, GetLastError, SetLastError},
    UI::{
        Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
        WindowsAndMessaging::{
            GWL_STYLE, GetWindowLongPtrW, IsWindow, SWP_HIDEWINDOW, SWP_NOMOVE, SWP_NOSIZE,
            SWP_SHOWWINDOW,
        },
    },
};

/// Borrow one validated peer without changing its native identity or allocating storage.
pub(super) fn update(
    callback: &Callback,
    parent: Option<&Registration<Callback>>,
    peer: &Peer,
    mode: PaintMode,
    control: AccessibleControl,
) -> Result<(), DesktopError> {
    super::state::update_visibility(
        peer.snapshot,
        control,
        &mut Visibility { callback, parent, peer, mode, control },
    )
}

/// Exact original native resources remain with Controls across every reentrant effect.
struct Visibility<'a> {
    callback: &'a Callback,
    parent: Option<&'a Registration<Callback>>,
    peer: &'a Peer,
    mode: PaintMode,
    control: AccessibleControl,
}
impl VisibilityPort for Visibility<'_> {
    fn live(&self) -> Result<(), DesktopError> {
        if self.parent.is_none_or(|parent| !parent.installed() || parent.destroyed())
            || self
                .peer
                .registration
                .as_ref()
                .is_none_or(|peer| !peer.installed() || peer.destroyed())
        {
            return Err(DesktopError::message(
                "update_accessible_visibility",
                "original callback registration retired",
            ));
        }
        // SAFETY: The creating thread retains the original peer and callback registrations.
        if !unsafe { IsWindow(Some(self.peer.hwnd)) }.as_bool() {
            return Err(DesktopError::message(
                "update_accessible_visibility",
                "original native peer retired",
            ));
        }
        Ok(())
    }
    fn action(&mut self, available: bool) -> Result<(), DesktopError> {
        self.callback.set_action(
            self.peer.hwnd,
            self.control.id,
            available.then(|| super::action(self.mode, self.control)).flatten(),
        )
    }
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError> {
        // SAFETY: This thread retains the original child. The return is previous state.
        let _previous = unsafe { EnableWindow(self.peer.hwnd, enabled) };
        self.live()?;
        // SAFETY: The exact original child was revalidated after native reentry.
        if unsafe { IsWindowEnabled(self.peer.hwnd) }.as_bool() != enabled {
            return Err(DesktopError::message(
                "update_accessible_enabled",
                "native enabled state differs",
            ));
        }
        Ok(())
    }
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError> {
        let flag = if visible { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
        // SAFETY: The original retained child stays on its creating UI thread; no focus or geometry change.
        unsafe {
            SetWindowPos(
                self.peer.hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | flag,
            )
        }
        .map_err(|cause| DesktopError::new("update_accessible_visibility", cause))?;
        self.live()?;
        // SAFETY: Observe the original child style after reentry. A hidden parent is independent.
        let (style, code) = unsafe {
            SetLastError(ERROR_SUCCESS);
            let style = GetWindowLongPtrW(self.peer.hwnd, GWL_STYLE) as u32;
            (style, GetLastError())
        };
        if style == 0 && code != ERROR_SUCCESS {
            return Err(DesktopError::new(
                "query_accessible_visibility",
                windows::core::Error::from(windows::core::HRESULT::from_win32(code.0)),
            ));
        }
        if style & WS_CHILD.0 == 0 || (style & WS_VISIBLE.0 != 0) != visible {
            return Err(DesktopError::message(
                "update_accessible_visibility",
                "native child visibility differs",
            ));
        }
        Ok(())
    }
}
