// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exclusive caption hover ownership invalidates both sides of every native transition.
use crate::DesktopError;
use std::cell::Cell;
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::Gdi::InvalidateRect,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::{IsWindowEnabled, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
        WindowsAndMessaging::{
            GetClientRect, WM_CANCELMODE, WM_CAPTURECHANGED, WM_ENABLE, WM_MOUSEMOVE, WM_NCDESTROY,
            WM_SHOWWINDOW,
        },
    },
};

/// Interpret child-local coordinates even while the native BUTTON owns capture.
pub(super) fn child(
    hovered: &Cell<HWND>,
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Result<(), DesktopError> {
    let enter = if message == WM_MOUSEMOVE {
        let mut rect = RECT::default();
        // SAFETY: This synchronous child callback retains the original peer HWND.
        unsafe { GetClientRect(hwnd, &raw mut rect) }
            .map_err(|cause| DesktopError::new("caption_hover_bounds", cause))?;
        let x = lparam.0 as u16 as i16 as i32;
        let y = (lparam.0 >> 16) as u16 as i16 as i32;
        // SAFETY: Query availability of the exact retained child before taking hover ownership.
        let inside = unsafe { IsWindowEnabled(hwnd) }.as_bool()
            && x >= rect.left
            && x < rect.right
            && y >= rect.top
            && y < rect.bottom;
        if inside && hovered.get() != hwnd {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                ..Default::default()
            };
            // SAFETY: Original subclass retains this peer throughout the synchronous native call.
            unsafe { TrackMouseEvent(&raw mut tracking) }
                .map_err(|cause| DesktopError::new("track_caption_hover", cause))?;
        }
        Some(inside)
    } else if matches!(message, WM_MOUSELEAVE | WM_CANCELMODE | WM_NCDESTROY)
        || matches!(message, WM_ENABLE | WM_SHOWWINDOW) && wparam.0 == 0
        || message == WM_CAPTURECHANGED && lparam.0 != 0 && lparam.0 != hwnd.0 as isize
    {
        Some(false)
    } else {
        None
    };
    if let Some(enter) = enter {
        change(hovered, hwnd, enter, |peer| {
            // SAFETY: Old and new ownership refer to original peers retained by this callback.
            if !unsafe { InvalidateRect(Some(peer), None, false) }.as_bool() {
                return Err(DesktopError::message(
                    "invalidate_caption_hover",
                    "InvalidateRect failed",
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Publish one exclusive identity before effects; stale leave messages cannot clear a new owner.
fn change(
    hovered: &Cell<HWND>,
    peer: HWND,
    enter: bool,
    mut invalidate: impl FnMut(HWND) -> Result<(), DesktopError>,
) -> Result<(), DesktopError> {
    let previous = hovered.get();
    let next = if enter {
        peer
    } else if previous == peer {
        HWND::default()
    } else {
        previous
    };
    if next == previous {
        return Ok(());
    }
    hovered.set(next);
    let mut failure = None;
    for dirty in [previous, next] {
        if !dirty.is_invalid()
            && let Err(cause) = invalidate(dirty)
        {
            DesktopError::retain_first(&mut failure, cause);
        }
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
#[path = "windows_caption_hover_tests.rs"]
mod tests;
