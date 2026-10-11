// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise actual Windows font ownership, bounds and stable native raster output.

use super::{LabelSurface, StatusText};
use crate::{LOCALES, Theme};
use tiny_skia::Pixmap;

/// Clip every label over prepainted pixels without changing interior native glyph coverage.
#[test]
fn all_labels_preserve_pixels_outside_status_widget() -> Result<(), Box<dyn std::error::Error>> {
    for locale in LOCALES {
        let mut text = StatusText::new(locale.id)?;
        for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
            let size = (558.0 * scale).round() as u32;
            let bounds = [
                (24.0 * scale).round() as u32,
                (486.0 * scale).round() as u32,
                (534.0 * scale).round() as u32,
                (530.0 * scale).round() as u32,
            ];
            for theme in [Theme::Dark, Theme::Light] {
                let mut target = Pixmap::new(size, size).ok_or("Cannot allocate clip test")?;
                target.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                let before = target.data().to_vec();
                let mut unclipped = target.clone();
                let runs = text.shape(locale.loading)?;
                let baseline =
                    runs.iter().map(|run| run.baseline.1).reduce(f32::max).ok_or("No glyphs")?;
                let mut surface = LabelSurface {
                    pixels: &mut unclipped,
                    bounds: [0, 0, size as i32, size as i32],
                };
                for run in &runs {
                    text.paint_run(&mut surface, run, theme, scale, 518.0 - baseline)?;
                }
                text.paint(&mut target, locale.loading, theme, scale)?;
                let mut visible = false;
                for (index, ((actual, original), expected)) in target
                    .data()
                    .chunks_exact(4)
                    .zip(before.chunks_exact(4))
                    .zip(unclipped.data().chunks_exact(4))
                    .enumerate()
                {
                    let x = index as u32 % size;
                    let y = index as u32 / size;
                    if x < bounds[0] || y < bounds[1] || x >= bounds[2] || y >= bounds[3] {
                        assert_eq!(actual, original, "{} scale {scale} at ({x},{y})", locale.id);
                    } else {
                        assert_eq!(actual, expected, "{} scale {scale} at ({x},{y})", locale.id);
                        visible |= actual != original;
                    }
                }
                assert!(visible, "{} scale {scale} has no interior glyph coverage", locale.id);
            }
        }
    }
    Ok(())
}

/// Every release-enabled label uses actual native font discovery and produces coverage.
#[test]
fn supported_labels_render_with_native_fallback() -> Result<(), Box<dyn std::error::Error>> {
    for locale in LOCALES {
        let mut text = StatusText::new(locale.id)?;
        let runs = text.shape(locale.loading)?;
        assert!(!runs.is_empty());
        assert!(runs.iter().all(|run| run.indices.iter().all(|glyph| *glyph != 0)));
        let mut target = Pixmap::new(1116, 1116).ok_or("Cannot allocate status test")?;
        text.paint(&mut target, locale.loading, Theme::Dark, 2.0)?;
        assert!(target.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert!(
            target
                .data()
                .chunks_exact(4)
                .all(|pixel| pixel[..3].iter().all(|channel| *channel <= pixel[3]))
        );
    }
    Ok(())
}

/// COM callback buffers and retained faces survive shaping and repeated owner teardown.
#[test]
fn repeated_font_owner_teardown_preserves_exact_raster() -> Result<(), Box<dyn std::error::Error>> {
    let mut previous = None;
    for _ in 0..3 {
        let mut text = StatusText::new("en")?;
        let mut target = Pixmap::new(698, 698).ok_or("Cannot allocate status test")?;
        text.paint(&mut target, "Loading...", Theme::Dark, 1.25)?;
        assert!(target.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
        if let Some(previous) = &previous {
            assert_eq!(target.data(), previous);
        }
        previous = Some(target.data().to_vec());
    }
    Ok(())
}

/// Invalid scales and oversized strings return errors without modifying pixels.
#[test]
fn invalid_status_input_preserves_target() -> Result<(), Box<dyn std::error::Error>> {
    assert!(StatusText::new(&"x".repeat(65)).is_err());
    assert!(StatusText::new("en\0ja").is_err());
    let mut text = StatusText::new("en")?;
    let mut target = Pixmap::new(558, 558).ok_or("Cannot allocate status test")?;
    for scale in [f32::NAN, f32::INFINITY, 0.0, -1.0, 4.01] {
        assert!(text.paint(&mut target, "Loading...", Theme::Dark, scale).is_err());
    }
    assert!(text.paint(&mut target, &"x".repeat(4097), Theme::Dark, 1.0).is_err());
    assert!(target.data().iter().all(|byte| *byte == 0));
    Ok(())
}
