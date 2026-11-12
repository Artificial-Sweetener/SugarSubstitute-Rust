// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render localized recovery text with composited alpha and native focus feedback.
use crate::DesktopError;
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, RECT, SIZE},
        Graphics::Gdi::{
            ANTIALIASED_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateFontW,
            DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE,
            DT_VCENTER, DeleteDC, DeleteObject, GetTextExtentPoint32W, HDC, HFONT,
            OUT_DEFAULT_PRECIS, SelectObject,
        },
        UI::Controls::{
            BufferedPaintSetAlpha, CS_ACTIVE, CloseThemeData, DTT_COMPOSITED, DTT_TEXTCOLOR,
            DTTOPTS, DrawThemeTextEx, OpenThemeData, WP_CAPTION,
        },
    },
    core::w,
};

/// Immutable native text policy for the localized recovery button.
pub(super) struct Style {
    pub(super) light_foreground: bool,
    pub(super) disabled: bool,
    /// Centre the words, for short action labels.
    pub(super) centered: bool,
    pub(super) dpi: u32,
}

/// Fonts and themes remain local to this UI-thread draw; names remain with the native HWND.
pub(super) fn draw(
    hwnd: HWND,
    dc: HDC,
    mut rect: RECT,
    text: &[u16],
    style: Style,
) -> Result<(), DesktopError> {
    let Style { light_foreground, disabled, centered, dpi } = style;
    if text.is_empty() || rect.right <= rect.left {
        return Ok(());
    }
    let font = font(dpi)?;
    // SAFETY: Keep the original object and restore it before deleting this owned font.
    let previous = unsafe { SelectObject(dc, font.into()) };
    let mut failure = if previous.is_invalid() {
        Some(DesktopError::message("select_caption_font", "SelectObject failed"))
    } else {
        // SAFETY: The parent/child remains retained; this local theme is closed before return.
        let theme = unsafe { OpenThemeData(Some(hwnd), w!("WINDOW")) };
        if theme.is_invalid() {
            Some(DesktopError::message("open_caption_theme", "OpenThemeData failed"))
        } else {
            let value = if disabled {
                128
            } else if light_foreground {
                255
            } else {
                0
            };
            let options = DTTOPTS {
                dwSize: size_of::<DTTOPTS>() as u32,
                dwFlags: DTT_COMPOSITED | DTT_TEXTCOLOR,
                crText: COLORREF(value * 0x010101),
                ..Default::default()
            };
            let format = DT_SINGLELINE
                | DT_VCENTER
                | DT_END_ELLIPSIS
                | if centered { DT_CENTER } else { DT_LEFT };
            // SAFETY: Text, destination and options remain live for this synchronous alpha-aware draw.
            let mut failure = unsafe {
                DrawThemeTextEx(
                    theme,
                    dc,
                    WP_CAPTION.0,
                    CS_ACTIVE.0,
                    text,
                    format,
                    &raw mut rect,
                    Some(&options),
                )
            }
            .map_err(|cause| DesktopError::new("draw_caption_text", cause))
            .err();
            // SAFETY: Retire the exact local theme on both draw outcomes.
            if let Err(cause) = unsafe { CloseThemeData(theme) } {
                DesktopError::retain_first(
                    &mut failure,
                    DesktopError::new("close_caption_theme", cause),
                );
            }
            failure
        }
    };
    if !previous.is_invalid() {
        // SAFETY: Restore the original borrowed object before deleting our selected font.
        if unsafe { SelectObject(dc, previous) } != font.into() {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::message("restore_caption_font", "SelectObject failed"),
            );
        }
    }
    // SAFETY: This exact font is no longer selected into the local buffer DC.
    if !unsafe { DeleteObject(font.into()) }.as_bool() {
        DesktopError::retain_first(
            &mut failure,
            DesktopError::message("delete_caption_font", "DeleteObject failed"),
        );
    }
    failure.map_or(Ok(()), Err)
}

/// The caption font, Segoe UI at 12 logical pixels, created for `dpi`. The caller deletes it.
fn font(dpi: u32) -> Result<HFONT, DesktopError> {
    // SAFETY: Creating a font object has no side effects beyond the returned owned handle.
    let font = unsafe {
        CreateFontW(
            -((12 * dpi / 96) as i32),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    };
    if font.is_invalid() {
        return Err(DesktopError::message("create_caption_font", "CreateFontW failed"));
    }
    Ok(font)
}

/// Device-pixel width of `text` in the caption font at `dpi`, measured in an owned memory DC.
pub(super) fn measure(text: &[u16], dpi: u32) -> Result<i32, DesktopError> {
    if text.is_empty() {
        return Ok(0);
    }
    let font = font(dpi)?;
    // SAFETY: An owned memory DC displays nothing; it is deleted on every path below.
    let dc = unsafe { CreateCompatibleDC(None) };
    let mut failure = None;
    let mut size = SIZE::default();
    if dc.is_invalid() {
        failure = Some(DesktopError::message("measure_caption_text", "CreateCompatibleDC failed"));
    } else {
        // SAFETY: The owned font is selected into the owned DC and restored before deletion.
        let previous = unsafe { SelectObject(dc, font.into()) };
        let measured = !previous.is_invalid()
            // SAFETY: Text and output live for this synchronous measurement.
            && unsafe { GetTextExtentPoint32W(dc, text, &raw mut size) }.as_bool();
        if !measured {
            failure =
                Some(DesktopError::message("measure_caption_text", "GetTextExtentPoint32W failed"));
        }
        // SAFETY: Restore the DC's original font before either object is deleted.
        if !previous.is_invalid() && unsafe { SelectObject(dc, previous) } != font.into() {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::message("restore_caption_font", "SelectObject failed"),
            );
        }
        // SAFETY: Delete exactly the memory DC created above.
        if !unsafe { DeleteDC(dc) }.as_bool() {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::message("measure_caption_text", "DeleteDC failed"),
            );
        }
    }
    // SAFETY: The font is no longer selected into any DC.
    if !unsafe { DeleteObject(font.into()) }.as_bool() {
        DesktopError::retain_first(
            &mut failure,
            DesktopError::message("delete_caption_font", "DeleteObject failed"),
        );
    }
    failure.map_or(Ok(size.cx), Err)
}

/// An alpha-aware narrow focus ring remains visible on glass and does not XOR prior frames.
pub(super) fn focus(
    dc: HDC,
    buffer: isize,
    rect: RECT,
    dark: bool,
    dpi: u32,
) -> Result<(), DesktopError> {
    use windows::Win32::Graphics::Gdi::{CreateSolidBrush, FillRect};
    let inset = ((3 * dpi + 48) / 96) as i32;
    let stroke = ((dpi + 48) / 96).max(1) as i32;
    let rect = RECT {
        left: rect.left + inset,
        top: rect.top + inset,
        right: rect.right - inset,
        bottom: rect.bottom - inset,
    };
    // SAFETY: Own this bounded native brush only for this synchronous memory-DC draw.
    let brush = unsafe { CreateSolidBrush(COLORREF(if dark { 0x00ffffff } else { 0x00181818 })) };
    if brush.is_invalid() {
        return Err(DesktopError::message("create_caption_focus", "CreateSolidBrush failed"));
    }
    let edges = [
        RECT { bottom: rect.top + stroke, ..rect },
        RECT { top: rect.bottom - stroke, ..rect },
        RECT { right: rect.left + stroke, ..rect },
        RECT { left: rect.right - stroke, ..rect },
    ];
    let mut failure = None;
    for edge in edges {
        // SAFETY: Each edge is bounded by this retained native control's current client bounds.
        if unsafe { FillRect(dc, &edge, brush) } == 0 {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::message("draw_caption_focus", "FillRect failed"),
            );
        }
        // SAFETY: GDI clears alpha; publish only the focus-ring edge as opaque.
        let alpha = unsafe { BufferedPaintSetAlpha(buffer, Some(&edge), 255) };
        if let Err(cause) = alpha {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::new("caption_focus_alpha", cause),
            );
        }
    }
    // SAFETY: Retire precisely the brush created by this paint, including after a failed draw.
    if !unsafe { DeleteObject(brush.into()) }.as_bool() {
        DesktopError::retain_first(
            &mut failure,
            DesktopError::message("delete_caption_focus", "DeleteObject failed"),
        );
    }
    failure.map_or(Ok(()), Err)
}
