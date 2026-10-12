// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify real palette, clipped shaped rectangles and premultiplied composition.
use super::*;

#[test]
fn palette_preserves_source_basics_extended_indices_rgb_and_defaults() {
    let expected = [
        0x17191c, 0xf66151, 0x33d17a, 0xe5a50a, 0x3584e4, 0xc061cb, 0x33c7de, 0xdeddda, 0x77767b,
        0xff7b72, 0x57e389, 0xf8e45c, 0x62a0ea, 0xdc8add, 0x5bc8af, 0xffffff,
    ];
    for (index, v) in expected.into_iter().enumerate() {
        assert_eq!(
            color(AnsiColor::Indexed(index as u8), Theme::Dark, false),
            Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
        );
    }
    for index in 0..=255 {
        assert!(color(AnsiColor::Indexed(index), Theme::Light, true).is_some());
    }
    for (index, rgb) in
        [(16, [0, 0, 0]), (21, [0, 0, 255]), (231, [255, 255, 255]), (232, [8; 3]), (255, [238; 3])]
    {
        assert_eq!(color(AnsiColor::Indexed(index), Theme::Dark, false), Some(rgb));
    }
    assert_eq!(color(AnsiColor::Rgb(12, 34, 56), Theme::Dark, false), Some([12, 34, 56]));
    assert_eq!(color(AnsiColor::Default, Theme::Dark, false), Some([255; 3]));
    assert_eq!(color(AnsiColor::Default, Theme::Light, false), Some([0; 3]));
    assert_eq!(color(AnsiColor::Default, Theme::Dark, true), None);
}

#[test]
fn actual_background_cursor_clips_separated_spans_and_respects_each_pixel_budget()
-> Result<(), Box<dyn std::error::Error>> {
    let mut pixels = Pixmap::new(558, 558).ok_or("Raster absent")?;
    let spec = ConsolePaintSpec {
        surface: PaintSpec { theme: Theme::Dark, scale: 1.0, width: 558, height: 558 },
        row: 0,
        rows: 10,
    };
    let spans = vec![
        Background { bounds: [-10.0, 350.0, 34.0, 600.0], color: [255, 0, 0] },
        Background { bounds: [526.0, 360.0, 700.0, 600.0], color: [0, 255, 0] },
    ];
    let mut paint = BackgroundPaint::new(spans, spec)?;
    let max = NonZeroUsize::new(7).ok_or("Budget absent")?;
    let mut before = 0;
    loop {
        let done = paint.step(&mut pixels, max)?;
        let after = pixels.data().chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(after - before <= 7);
        before = after;
        if done {
            break;
        }
    }
    assert_eq!(before, 6 * 16);
    assert_eq!(pixels.pixel(29, 368).ok_or("Pixel absent")?.alpha(), 0);
    assert_eq!(pixels.pixel(30, 367).ok_or("Pixel absent")?.alpha(), 0);
    assert_eq!(pixels.pixel(30, 368).ok_or("Pixel absent")?.red(), 255);
    assert_eq!(pixels.pixel(528, 368).ok_or("Pixel absent")?.alpha(), 0);
    assert_eq!(pixels.pixel(526, 383).ok_or("Pixel absent")?.green(), 255);
    assert_eq!(pixels.pixel(526, 384).ok_or("Pixel absent")?.alpha(), 0);
    Ok(())
}

#[test]
fn invalid_shaped_geometry_rejects_before_painting_and_blending_stays_premultiplied() {
    let spec = ConsolePaintSpec {
        surface: PaintSpec { theme: Theme::Dark, scale: 1.0, width: 558, height: 558 },
        row: 0,
        rows: 1,
    };
    assert!(
        BackgroundPaint::new(
            vec![Background { bounds: [0.0, 0.0, f32::NAN, 3.0], color: [0; 3] }],
            spec
        )
        .is_err()
    );
    let mut p = [0; 4];
    composite(&mut p, [255, 0, 128], 128);
    assert_eq!(p, [128, 0, 64, 128]);
    composite(&mut p, [0, 255, 0], 64);
    assert!(p[..3].iter().all(|v| *v <= p[3]));
}
