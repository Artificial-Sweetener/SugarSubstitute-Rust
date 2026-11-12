// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Checked availability effects for borrowed native child controls on their creating thread.
use super::NativeUiError;
use windows::Win32::{
    Foundation::{ERROR_SUCCESS, GetLastError, HWND, SetLastError},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
        WindowsAndMessaging::{
            GWL_STYLE, GetWindowLongPtrW, GetWindowThreadProcessId, SWP_HIDEWINDOW, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, WS_CHILD,
            WS_VISIBLE,
        },
    },
};

/// Set the OS enabled bit on an owned child without changing its visibility or geometry.
/// The caller retains original child/callback custody and revalidates it after this call,
/// before restoring callback admission. Win32 can reenter the caller's window procedures.
/// # Errors
/// Rejects foreign-thread/non-child handles and preserves failed native postconditions.
pub fn set_enabled(window: HWND, enabled: bool) -> Result<(), NativeUiError> {
    child_style(window)?;
    // SAFETY: The creating thread's child was validated; caller retains its original custody.
    let _previous = unsafe { EnableWindow(window, enabled) };
    child_style(window)?;
    // SAFETY: Passive query of the same still-live affine child after native reentry.
    if unsafe { IsWindowEnabled(window) }.as_bool() != enabled {
        return Err(NativeUiError::message("widget_enabled", "native enabled state differs"));
    }
    Ok(())
}

/// Change the child's own visible bit without moving, activating or restacking it.
/// A hidden parent does not change this child-state observation. The caller retains and
/// revalidates original child/callback custody before subsequent effects or activation.
/// # Errors
/// Rejects foreign-thread/non-child handles and preserves native positioning/query failures.
pub fn set_visible(window: HWND, visible: bool) -> Result<(), NativeUiError> {
    child_style(window)?;
    let flag = if visible { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
    // SAFETY: Retained creating-thread child; the flags prohibit activation, geometry and Z changes.
    unsafe {
        SetWindowPos(
            window,
            None,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | flag,
        )
    }
    .map_err(|cause| NativeUiError::new("widget_visibility", cause))?;
    let style = child_style(window)?;
    if (style & WS_VISIBLE.0 != 0) != visible {
        return Err(NativeUiError::message("widget_visibility", "native child visibility differs"));
    }
    Ok(())
}

/// Observe only a live child belonging to the calling UI thread.
fn child_style(window: HWND) -> Result<u32, NativeUiError> {
    // SAFETY: Handle/thread getters perform no pointer dereference or ownership transfer.
    if unsafe { GetWindowThreadProcessId(window, None) } != unsafe { GetCurrentThreadId() } {
        return Err(NativeUiError::message(
            "widget_thread",
            "native child belongs to another thread or is retired",
        ));
    }
    // SAFETY: Query the validated thread-affine handle; zero is disambiguated with LastError.
    let (style, code) = unsafe {
        SetLastError(ERROR_SUCCESS);
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        (style, GetLastError())
    };
    if style == 0 && code != ERROR_SUCCESS {
        return Err(NativeUiError::new(
            "widget_style",
            windows::core::Error::from(windows::core::HRESULT::from_win32(code.0)),
        ));
    }
    if style & WS_CHILD.0 == 0 {
        return Err(NativeUiError::message("widget_child", "native peer is not a child control"));
    }
    Ok(style)
}
