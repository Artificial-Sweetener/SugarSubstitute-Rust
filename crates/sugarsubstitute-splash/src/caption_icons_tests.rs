// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate packaged caption alpha, tinting, device phase and scale rejection.

use super::{ASSETS, CaptionIcon, CaptionIcons};
use sugarsubstitute_ui::bitmap::Bitmap;

/// Require every resource to be bounded, nonempty white-alpha artwork.
#[test]
fn every_scale_has_valid_transparent_glyphs() -> Result<(), Box<dyn std::error::Error>> {
    for (scale, console, close) in ASSETS {
        for bytes in [console, close] {
            let glyph = Bitmap::decode_png(bytes)?;
            assert_eq!(glyph.width(), (46.0 * scale).round() as u32);
            assert_eq!(glyph.height(), (32.0 * scale).round() as u32);
            assert_eq!(glyph.pixel(0, 0).map(|p| p.alpha()), Some(0));
            assert!(glyph.pixels().iter().any(|p| p.alpha() > 0));
            assert!(
                glyph.pixels().iter().all(|p| p.red() == p.alpha()
                    && p.green() == p.alpha()
                    && p.blue() == p.alpha())
            );
        }
    }
    Ok(())
}

/// Preserve exact resource bytes at known scales and floor-based global placement.
#[test]
fn exact_scale_preserves_glyph_and_tint() -> Result<(), Box<dyn std::error::Error>> {
    let mut icons = CaptionIcons::new();
    for (scale, console, close) in ASSETS {
        for (kind, bytes, offset) in
            [(CaptionIcon::Console, console, 466.0), (CaptionIcon::Close, close, 512.0)]
        {
            let mut white =
                Bitmap::new((558.0 * scale).round() as u32, (32.0 * scale).round() as u32)
                    .ok_or("allocation")?;
            icons.draw(&mut white, scale, kind, [255; 3])?;
            let source = Bitmap::decode_png(bytes)?;
            let x = (offset * scale).floor() as u32;
            for row in 0..source.height() {
                for column in 0..source.width() {
                    assert_eq!(white.pixel(x + column, row), source.pixel(column, row));
                }
            }
            let mut black = white.clone();
            black.fill(tiny_skia::Color::TRANSPARENT);
            icons.draw(&mut black, scale, kind, [0; 3])?;
            assert!(black.pixels().iter().all(|p| p.red() == 0 && p.green() == 0 && p.blue() == 0));
            assert!(white.pixels().iter().zip(black.pixels()).all(|(a, b)| a.alpha() == b.alpha()));
        }
    }
    Ok(())
}

/// Reject invalid input without painting and support unlisted device scales.
#[test]
fn arbitrary_scales_remain_bounded_and_invalid_input_is_atomic()
-> Result<(), Box<dyn std::error::Error>> {
    let mut icons = CaptionIcons::new();
    let mut target = Bitmap::new(1228, 71).ok_or("allocation")?;
    for scale in [f64::NAN, f64::INFINITY, 0.0, 4.1] {
        assert!(icons.draw(&mut target, scale, CaptionIcon::Close, [255; 3]).is_err());
        assert!(target.pixels().iter().all(|p| p.alpha() == 0));
    }
    icons.draw(&mut target, 2.2, CaptionIcon::Close, [255; 3])?;
    assert!(target.pixels().iter().any(|p| p.alpha() > 0));
    assert!(
        (0..target.height())
            .all(|y| (0..1126).all(|x| target.pixel(x, y).is_some_and(|p| p.alpha() == 0)))
    );
    Ok(())
}
