// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint the frameless Main caption without replacing its native accessible button lifetime.
use super::Callback;
use crate::{DesktopError, NativeMaterial};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{BeginPaint, EndPaint, HDC, PAINTSTRUCT},
    UI::{
        Controls::{
            BP_PAINTPARAMS, BPBF_TOPDOWNDIB, BPPF_ERASE, BeginBufferedPaint, BufferedPaintInit,
            BufferedPaintUnInit, DRAWITEMSTRUCT, EndBufferedPaint, ODS_DISABLED, ODS_FOCUS,
            ODS_NOFOCUSRECT, ODS_SELECTED,
        },
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            GetClientRect, GetParent, GetWindowTextW, IsZoomed, WM_DRAWITEM, WM_ERASEBKGND,
            WM_PAINT,
        },
    },
};
#[path = "windows_caption_glyphs.rs"]
mod glyphs;
#[path = "windows_caption_hover.rs"]
mod hover;
#[path = "windows_caption_pixels.rs"]
mod pixels;
#[path = "windows_caption_text.rs"]
mod text;

/// Immutable surface palette borrowed by the original parent and peer callbacks.
#[derive(Clone, Copy)]
pub(crate) struct Caption {
    pub(crate) dark: bool,
    pub(crate) material: NativeMaterial,
}

/// Route only this configured Main's parent paint and exact registered owner-drawn children.
pub(super) fn parent(
    callback: &Callback,
    hwnd: HWND,
    message: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    let caption = callback.caption.get()?;
    match message {
        WM_ERASEBKGND => Some(LRESULT(1)),
        WM_PAINT => {
            report(paint_parent(hwnd, caption));
            Some(LRESULT(0))
        }
        WM_DRAWITEM if lparam.0 != 0 => {
            // SAFETY: WM_DRAWITEM supplies a synchronous DRAWITEMSTRUCT borrowed for this message.
            let item = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            let registered = callback.controls.try_borrow().is_ok_and(|controls| {
                controls
                    .iter()
                    .any(|(peer, id, _)| *peer == item.hwndItem && u32::from(*id) == item.CtlID)
            });
            if !registered {
                return None;
            }
            report(paint_button(item, caption, callback.hovered.get() == item.hwndItem));
            Some(LRESULT(1))
        }
        _ => None,
    }
}

/// Keep hover ownership exclusive while Win32 retains capture, pressed and accessibility state.
pub(super) fn child(callback: &Callback, hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) {
    if callback.caption.get().is_some() {
        report(hover::child(&callback.hovered, hwnd, message, wparam, lparam));
    }
}

/// Clear the native backing plane; the full-window browser owns its complete frame presentation.
fn paint_parent(hwnd: HWND, caption: Caption) -> Result<(), DesktopError> {
    let mut paint = PAINTSTRUCT::default();
    // SAFETY: The original parent callback owns this HWND and balances BeginPaint on every path.
    let dc = unsafe { BeginPaint(hwnd, &raw mut paint) };
    let mut failure = if dc.is_invalid() {
        Some(DesktopError::message("begin_caption_paint", "BeginPaint failed"))
    } else {
        (|| {
            let mut bounds = RECT::default();
            // SAFETY: A valid retained HWND and live scalar output are borrowed synchronously.
            unsafe { GetClientRect(hwnd, &raw mut bounds) }
                .map_err(|cause| DesktopError::new("caption_client_bounds", cause))?;
            if bounds.right <= bounds.left || bounds.bottom <= bounds.top {
                return Ok(());
            }
            buffered(dc, bounds, |_buffer, handle| {
                background(handle, bounds, caption, false, false, false, 96)?;
                Ok(())
            })
        })()
        .err()
    };
    // SAFETY: Complete this invocation's BeginPaint, even when painting failed.
    if !unsafe { EndPaint(hwnd, &paint) }.as_bool() {
        DesktopError::retain_first(
            &mut failure,
            DesktopError::message("end_caption_paint", "EndPaint failed"),
        );
    }
    failure.map_or(Ok(()), Err)
}

/// Paint original vector geometry while preserving localized native accessible names.
fn paint_button(item: &DRAWITEMSTRUCT, caption: Caption, hot: bool) -> Result<(), DesktopError> {
    let pressed = item.itemState.0 & ODS_SELECTED.0 != 0;
    let disabled = item.itemState.0 & ODS_DISABLED.0 != 0;
    // SAFETY: The registered child and its parent remain owned by the active callback.
    let (parent, dpi) = unsafe { (GetParent(item.hwndItem), GetDpiForWindow(item.hwndItem)) };
    let parent = parent.map_err(|cause| DesktopError::new("caption_button_parent", cause))?;
    // SAFETY: Exact parent remains retained; this query has no side effects.
    let maximized = unsafe { IsZoomed(parent) }.as_bool();
    buffered(item.hDC, item.rcItem, |buffer, handle| {
        background(handle, item.rcItem, caption, hot, pressed, item.CtlID == 3, dpi)?;
        let light = caption.dark || item.CtlID == 3 && (hot || pressed);
        if (1..=3).contains(&item.CtlID) {
            pixels::with_pixels(handle, item.rcItem, |pixels| {
                glyphs::draw(pixels, item.CtlID, maximized, dpi, if light { 255 } else { 0 })
            })?;
        } else {
            let mut label = [0_u16; 129];
            // SAFETY: Read the original localized retry peer name into a bounded buffer.
            let length = unsafe { GetWindowTextW(item.hwndItem, &mut label) }.max(0) as usize;
            text::draw(
                item.hwndItem,
                buffer,
                item.rcItem,
                &label[..length],
                text::Style { light_foreground: light, disabled, dpi },
            )?;
        }
        if item.itemState.0 & ODS_FOCUS.0 != 0 && item.itemState.0 & ODS_NOFOCUSRECT.0 == 0 {
            text::focus(buffer, handle, item.rcItem, caption.dark, dpi)?;
        }
        Ok(())
    })
}

/// Preserve the original translucent feedback wash over the complete Mica backing plane.
fn background(
    buffer: isize,
    rect: RECT,
    caption: Caption,
    hot: bool,
    pressed: bool,
    close: bool,
    dpi: u32,
) -> Result<(), DesktopError> {
    let rgba = if close && (hot || pressed) {
        if pressed { [241, 112, 122, 255] } else { [232, 17, 35, 255] }
    } else {
        let value = if caption.dark { 255 } else { 0 };
        [
            value,
            value,
            value,
            if pressed {
                51
            } else if hot {
                26
            } else {
                0
            },
        ]
    };
    pixels::with_pixels(buffer, rect, |pixels| {
        let value = if caption.dark { 32 } else { 243 };
        pixels.fill(if caption.material == NativeMaterial::Mica {
            [0; 4]
        } else {
            [value, value, value, 255]
        });
        if close && (hot || pressed) {
            glyphs::fill_close_background(pixels, dpi, [rgba[0], rgba[1], rgba[2]])?;
        } else {
            for y in 0..pixels.height {
                for x in 0..pixels.width {
                    pixels.over(x, y, rgba[0], rgba[3]);
                }
            }
        }
        Ok(())
    })
}

/// Balance thread-affine UxTheme resources and never publish a partially failed caption frame.
fn buffered(
    dc: HDC,
    rect: RECT,
    draw: impl FnOnce(HDC, isize) -> Result<(), DesktopError>,
) -> Result<(), DesktopError> {
    // SAFETY: Initialize this calling UI thread's buffered-paint reference.
    unsafe { BufferedPaintInit() }
        .map_err(|cause| DesktopError::new("initialize_caption_buffer", cause))?;
    let params = BP_PAINTPARAMS {
        cbSize: size_of::<BP_PAINTPARAMS>() as u32,
        dwFlags: BPPF_ERASE,
        ..Default::default()
    };
    let mut target = HDC::default();
    // SAFETY: This bounded target rectangle and output remain live until EndBufferedPaint.
    let buffer =
        unsafe { BeginBufferedPaint(dc, &rect, BPBF_TOPDOWNDIB, Some(&params), &raw mut target) };
    let mut failure = if buffer == 0 || target.is_invalid() {
        Some(DesktopError::message("begin_caption_buffer", "BeginBufferedPaint failed"))
    } else {
        draw(target, buffer).err()
    };
    if buffer != 0 {
        // SAFETY: End precisely this retained paint and commit only fully prepared content.
        if let Err(cause) = unsafe { EndBufferedPaint(buffer, failure.is_none()) } {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::new("end_caption_buffer", cause),
            );
        }
    }
    // SAFETY: Balance the successful initialization even if buffer construction/drawing failed.
    if let Err(cause) = unsafe { BufferedPaintUnInit() } {
        DesktopError::retain_first(
            &mut failure,
            DesktopError::new("release_caption_buffer", cause),
        );
    }
    failure.map_or(Ok(()), Err)
}

/// Native procedures retain paint failures without unwinding across Win32.
fn report(result: Result<(), DesktopError>) {
    if let Err(cause) = result {
        tracing::error!(operation="paint_main_caption", cause=%cause, "Native caption painting failed");
    }
}

#[cfg(test)]
#[path = "windows_caption_tests.rs"]
mod tests;
