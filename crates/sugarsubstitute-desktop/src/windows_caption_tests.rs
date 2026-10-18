// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect actual prepublication caption alpha and installed system-font glyph output.
use super::*;
use windows::Win32::{
    Graphics::Gdi::{CreateCompatibleDC, DeleteDC, RGBQUAD},
    UI::Controls::GetBufferedPaintBits,
};

/// Real glyph rasterization must survive DWM composition without making the Mica caption opaque.
#[test]
fn native_caption_pixels_keep_glass_transparent_and_glyphs_visible()
-> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: An owned memory DC cannot display a window or interact with the user's desktop.
    let dc = unsafe { CreateCompatibleDC(None) };
    assert!(!dc.is_invalid());
    let mut failure = None;
    for dark in [false, true] {
        for (dpi, height) in [(96, 40), (168, 70)] {
            assert_eq!(caption_height(dpi), height);
            let rect = RECT { left: 0, top: 0, right: (48 * dpi / 96) as i32, bottom: height };
            for (id, maximized) in [(1, false), (2, false), (2, true), (3, false)] {
                let symbol = glyph(id, maximized).ok_or("caption glyph absent")?;
                for (material, hot, pressed, focus) in [
                    (NativeMaterial::Mica, false, false, false),
                    (NativeMaterial::Mica, true, false, false),
                    (NativeMaterial::Mica, true, true, true),
                    (NativeMaterial::ThemedFallback, false, false, false),
                ] {
                    let result = buffered(dc, rect, |target, buffer| {
                        background(
                            target,
                            buffer,
                            &rect,
                            Caption { dark, material },
                            hot,
                            pressed,
                            id == 3,
                        )?;
                        text::draw(
                            HWND::default(),
                            target,
                            rect,
                            &[symbol],
                            text::Style {
                                icon: true,
                                light_foreground: dark || id == 3 && (hot || pressed),
                                disabled: false,
                                dpi,
                            },
                        )?;
                        if focus {
                            text::focus(target, buffer, rect, dark, dpi)?;
                        }
                        inspect(buffer, rect, hot || pressed || material != NativeMaterial::Mica)
                    });
                    if let Err(cause) = result {
                        DesktopError::retain_first(&mut failure, cause);
                    }
                }
            }
        }
    }
    // SAFETY: Every buffered draw has retired before this test deletes its original memory DC.
    if !unsafe { DeleteDC(dc) }.as_bool() {
        DesktopError::retain_first(
            &mut failure,
            DesktopError::message("caption_fixture_delete_dc", "DeleteDC failed"),
        );
    }
    failure.map_or(Ok(()), |cause| Err(cause.into()))
}

/// Read UxTheme's live top-down buffer before any screenshot or window repaint can change it.
fn inspect(buffer: isize, rect: RECT, opaque: bool) -> Result<(), DesktopError> {
    let mut bits: *mut RGBQUAD = std::ptr::null_mut();
    let mut stride = 0;
    // SAFETY: The observer runs within buffered's retained native paint transaction.
    unsafe { GetBufferedPaintBits(buffer, &raw mut bits, &raw mut stride) }
        .map_err(|cause| DesktopError::new("caption_fixture_bits", cause))?;
    if bits.is_null() || stride < rect.right || rect.bottom <= 0 {
        return Err(DesktopError::message("caption_fixture_shape", "Invalid buffer shape"));
    }
    // SAFETY: UxTheme guarantees stride RGBQUADs for each row of this exact requested rectangle.
    let first = unsafe { *bits };
    let expected = if opaque { 255 } else { 0 };
    if first.rgbReserved != expected {
        return Err(DesktopError::message(
            "caption_fixture_background",
            "Caption backdrop alpha differs",
        ));
    }
    let mut glyph_pixels = 0;
    for y in 0..rect.bottom as usize {
        // SAFETY: Stride and positive shape were validated against the native requested buffer.
        let row = unsafe {
            std::slice::from_raw_parts(bits.add(y * stride as usize), rect.right as usize)
        };
        for pixel in row {
            if !opaque && pixel.rgbReserved != 0 {
                glyph_pixels += 1;
            }
            if opaque && pixel.rgbReserved != 255 {
                return Err(DesktopError::message(
                    "caption_fixture_interaction",
                    "Interaction surface lost opacity",
                ));
            }
            if pixel.rgbRed > pixel.rgbReserved
                || pixel.rgbGreen > pixel.rgbReserved
                || pixel.rgbBlue > pixel.rgbReserved
            {
                return Err(DesktopError::message(
                    "caption_fixture_premultiplication",
                    "Caption pixels are not premultiplied",
                ));
            }
        }
    }
    if !opaque && glyph_pixels < 4 {
        return Err(DesktopError::message(
            "caption_fixture_glyph",
            "Native caption glyph is transparent",
        ));
    }
    Ok(())
}
