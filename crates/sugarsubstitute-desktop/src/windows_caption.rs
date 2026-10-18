// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint the frameless Main caption without replacing its native accessible button lifetime.
use super::Callback;
use crate::{DesktopError, NativeMaterial};
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BLACK_BRUSH, BeginPaint, DC_BRUSH, EndPaint, FillRect, GetStockObject, HBRUSH, HDC,
        InvalidateRect, PAINTSTRUCT, SetDCBrushColor,
    },
    UI::{
        Controls::{
            BP_PAINTPARAMS, BPBF_TOPDOWNDIB, BPPF_ERASE, BeginBufferedPaint, BufferedPaintInit,
            BufferedPaintSetAlpha, BufferedPaintUnInit, DRAWITEMSTRUCT, EndBufferedPaint,
            ODS_DISABLED, ODS_FOCUS, ODS_NOFOCUSRECT, ODS_SELECTED, WM_MOUSELEAVE,
        },
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
        WindowsAndMessaging::{
            GetClientRect, GetParent, GetWindowTextW, IsZoomed, WM_DRAWITEM, WM_ERASEBKGND,
            WM_MOUSEMOVE, WM_PAINT,
        },
    },
};
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

/// Native BUTTON keeps capture/keyboard/pressed state; this owner adds bounded hover invalidation.
pub(super) fn child(callback: &Callback, hwnd: HWND, message: u32) {
    if callback.caption.get().is_none() {
        return;
    }
    if message == WM_MOUSEMOVE && callback.hovered.get() != hwnd {
        let mut tracking = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            ..Default::default()
        };
        // SAFETY: Original subclass retains this child throughout the synchronous native call.
        match unsafe { TrackMouseEvent(&raw mut tracking) } {
            Ok(()) => {
                callback.hovered.set(hwnd);
                // SAFETY: Invalidate only this retained child; no unrelated windows are touched.
                if !unsafe { InvalidateRect(Some(hwnd), None, false) }.as_bool() {
                    report(Err(DesktopError::message(
                        "invalidate_caption_hover",
                        "InvalidateRect failed",
                    )));
                }
            }
            Err(cause) => report(Err(DesktopError::new("track_caption_hover", cause))),
        }
    } else if message == WM_MOUSELEAVE && callback.hovered.get() == hwnd {
        callback.hovered.set(HWND::default());
        // SAFETY: The exact original child remains retained by this installed callback.
        if !unsafe { InvalidateRect(Some(hwnd), None, false) }.as_bool() {
            report(Err(DesktopError::message("invalidate_caption_leave", "InvalidateRect failed")));
        }
    }
}

/// Clear exactly the caption strip; the browser remains the authoritative owner below it.
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
            // SAFETY: This exact window remains retained and DPI is read without changing context.
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            bounds.bottom = bounds.bottom.min(caption_height(dpi));
            if bounds.right <= bounds.left || bounds.bottom <= bounds.top {
                return Ok(());
            }
            buffered(dc, bounds, |buffer, handle| {
                background(buffer, handle, &bounds, caption, false, false, false)?;
                let mut title = [0_u16; 129];
                // SAFETY: This bounded stack buffer receives only our own parent's localized title.
                let length = unsafe { GetWindowTextW(hwnd, &mut title) }.max(0) as usize;
                let mut title_bounds = bounds;
                title_bounds.left += (12 * dpi / 96) as i32;
                title_bounds.right -= (156 * dpi / 96) as i32;
                text::draw(
                    hwnd,
                    buffer,
                    title_bounds,
                    &title[..length],
                    text::Style {
                        icon: false,
                        light_foreground: caption.dark,
                        disabled: false,
                        dpi,
                    },
                )
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

/// Paint icons from the system Fluent symbol font while preserving localized HWND names.
fn paint_button(item: &DRAWITEMSTRUCT, caption: Caption, hot: bool) -> Result<(), DesktopError> {
    let pressed = item.itemState.0 & ODS_SELECTED.0 != 0;
    let disabled = item.itemState.0 & ODS_DISABLED.0 != 0;
    // SAFETY: The registered child and its parent remain owned by the active callback.
    let (parent, dpi) = unsafe { (GetParent(item.hwndItem), GetDpiForWindow(item.hwndItem)) };
    let parent = parent.map_err(|cause| DesktopError::new("caption_button_parent", cause))?;
    // SAFETY: Exact parent remains retained; this query has no side effects.
    let maximized = unsafe { IsZoomed(parent) }.as_bool();
    let glyph = glyph(item.CtlID, maximized);
    let mut label = [0_u16; 129];
    let content = if let Some(ref icon) = glyph {
        std::slice::from_ref(icon)
    } else {
        // SAFETY: Read only the original localized retry peer name into a bounded buffer.
        let length = unsafe { GetWindowTextW(item.hwndItem, &mut label) }.max(0) as usize;
        &label[..length]
    };
    buffered(item.hDC, item.rcItem, |buffer, handle| {
        background(buffer, handle, &item.rcItem, caption, hot, pressed, item.CtlID == 3)?;
        text::draw(
            item.hwndItem,
            buffer,
            item.rcItem,
            content,
            text::Style {
                icon: glyph.is_some(),
                light_foreground: caption.dark || item.CtlID == 3 && (hot || pressed),
                disabled,
                dpi,
            },
        )?;
        if item.itemState.0 & ODS_FOCUS.0 != 0 && item.itemState.0 & ODS_NOFOCUSRECT.0 == 0 {
            text::focus(buffer, handle, item.rcItem, caption.dark, dpi)?;
        }
        Ok(())
    })
}

/// Match native caption metrics independently of physical scaling and current restore state.
fn caption_height(dpi: u32) -> i32 {
    ((40 * dpi + 48) / 96) as i32
}
/// These system-font PUA mappings are defined by Microsoft's Segoe Fluent Icons contract.
fn glyph(id: u32, maximized: bool) -> Option<u16> {
    match id {
        1 => Some(0xe921),
        2 => Some(if maximized { 0xe923 } else { 0xe922 }),
        3 => Some(0xe8bb),
        _ => None,
    }
}

/// Keep Mica transparent in the rest state and publish opaque interaction/fallback surfaces.
fn background(
    dc: HDC,
    buffer: isize,
    rect: &RECT,
    caption: Caption,
    hot: bool,
    pressed: bool,
    close: bool,
) -> Result<(), DesktopError> {
    let color = if close && (hot || pressed) {
        if pressed { COLORREF(0x002323b8) } else { COLORREF(0x002311c4) }
    } else if hot || pressed || caption.material != NativeMaterial::Mica {
        let value = if caption.dark {
            if pressed {
                48
            } else if hot {
                58
            } else {
                32
            }
        } else if pressed {
            209
        } else if hot {
            228
        } else {
            243
        };
        COLORREF(value * 0x010101)
    } else {
        COLORREF(0)
    };
    // SAFETY: Buffer DC is local to this paint and stock brushes remain system owned.
    unsafe {
        if SetDCBrushColor(dc, color).0 == u32::MAX {
            return Err(DesktopError::message(
                "color_caption_background",
                "SetDCBrushColor failed",
            ));
        }
        let brush = if color.0 == 0 {
            HBRUSH(GetStockObject(BLACK_BRUSH).0)
        } else {
            HBRUSH(GetStockObject(DC_BRUSH).0)
        };
        if FillRect(dc, rect, brush) == 0 {
            return Err(DesktopError::message("fill_caption_background", "FillRect failed"));
        }
        BufferedPaintSetAlpha(buffer, Some(rect), if color.0 == 0 { 0 } else { 255 })
            .map_err(|cause| DesktopError::new("caption_background_alpha", cause))
    }
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
