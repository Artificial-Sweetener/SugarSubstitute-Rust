// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Draw native focus feedback on the caption's window buttons.
use crate::DesktopError;
use windows::Win32::{
    Foundation::{COLORREF, RECT},
    Graphics::Gdi::{DeleteObject, HDC},
    UI::Controls::BufferedPaintSetAlpha,
};

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
