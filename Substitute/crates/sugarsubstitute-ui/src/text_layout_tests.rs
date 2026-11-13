// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify reusable label geometry independently of any product surface.

use super::*;
use crate::bitmap::Bitmap;
use std::{error::Error, num::NonZeroUsize};

fn spec() -> LabelPaintSpec {
    LabelPaintSpec {
        surface: PaintSpec { theme: Theme::Dark, scale: 1.0, width: 420, height: 180 },
        layout: LabelLayout {
            origin: [20.0, 20.0],
            extent: [280.0, 80.0],
            clip: Some([20.0, 20.0, 300.0, 100.0]),
            anchor: VerticalAnchor::Top,
            alignment: TextAlignment::Leading,
            metrics: crate::text::TextMetrics::GdiCompatible,
            font_size: 22.0,
            line_height: 29.0,
            weight: 400,
            color: [255, 64, 137, 255],
        },
    }
}

fn paint(spec: LabelPaintSpec) -> Result<Bitmap, Box<dyn Error>> {
    let mut owner = crate::text::FontOwner::new("en")?;
    let mut paint = owner.prepare_offline("Choose your language", spec)?;
    let mut pixels = Bitmap::new(spec.surface.width, spec.surface.height).ok_or("Target absent")?;
    while !paint.step(&mut pixels, NonZeroUsize::new(17).ok_or("Budget absent")?)? {}
    Ok(pixels)
}

#[test]
fn label_translation_preserves_glyphs_and_moves_the_clip_with_the_surface()
-> Result<(), Box<dyn Error>> {
    let first = spec();
    let mut second = first;
    second.layout.origin[0] += 17.0;
    second.layout.origin[1] += 11.0;
    second.layout.clip =
        first.layout.clip.map(|[l, t, r, b]| [l + 17.0, t + 11.0, r + 17.0, b + 11.0]);
    let before = paint(first)?;
    let after = paint(second)?;
    assert!(before.data().iter().any(|v| *v != 0));
    for y in 0..180 {
        for x in 0..420 {
            let expected = if x >= 17 && y >= 11 { before.pixel(x - 17, y - 11) } else { None };
            let actual = after.pixel(x, y).ok_or("Pixel absent")?;
            assert_eq!(actual.alpha(), expected.map_or(0, |p| p.alpha()), "{x},{y}");
            if let Some(expected) = expected {
                assert_eq!(actual, expected, "{x},{y}");
            }
        }
    }
    Ok(())
}

#[test]
fn label_alignment_and_font_size_are_owned_by_the_supplied_layout() -> Result<(), Box<dyn Error>> {
    let left = paint(spec())?;
    let mut centered = spec();
    centered.layout.alignment = TextAlignment::Center;
    let center = paint(centered)?;
    assert_ne!(left.data(), center.data());
    centered.layout.font_size = 14.0;
    assert_ne!(center.data(), paint(centered)?.data());
    Ok(())
}

#[test]
fn invalid_label_geometry_is_rejected_before_font_or_target_work() {
    let base = spec();
    for invalid in [f32::NAN, f32::INFINITY, -1.0, 0.0, 8193.0] {
        let mut value = base;
        value.layout.extent[0] = invalid;
        assert!(value.validate("hello").is_err());
    }
    let mut value = base;
    value.layout.clip = Some([10.0, 0.0, 9.0, 10.0]);
    assert!(value.validate("hello").is_err());
    value = base;
    value.layout.line_height = 1.0;
    assert!(value.validate("hello").is_err());
    value = base;
    value.layout.weight = 0;
    assert!(value.validate("hello").is_err());
}

#[test]
fn measured_natural_lines_match_their_painted_spacing() -> Result<(), Box<dyn Error>> {
    let mut owner = crate::text::FontOwner::new("en")?;
    let mut spec = spec();
    spec.layout.metrics = TextMetrics::Natural;
    spec.layout.origin = [0.0, 0.0];
    spec.layout.extent = [280.0, 10.0];
    spec.layout.clip = None;
    spec.layout.line_height = 40.0;
    let message = "Hello\nHello\nHello";
    let size = owner.measure(message, spec, None)?;
    assert_eq!(size.lines, 3);
    assert!((size.height - 120.0).abs() < 0.01);
    let mut cursor = owner.prepare_offline(message, spec)?;
    let mut pixels = Bitmap::new(420, 180).ok_or("Target absent")?;
    while !cursor.step(&mut pixels, NonZeroUsize::MIN.saturating_add(32767))? {}
    let stride = pixels.width() as usize * 4;
    for line in 0..3 {
        let rows = &pixels.data()[line * 40 * stride..(line + 1) * 40 * stride];
        assert!(rows.chunks_exact(4).any(|pixel| pixel[3] > 0));
        assert_eq!(&pixels.data()[..40 * stride], rows);
    }
    assert!(pixels.data()[120 * stride..].iter().all(|byte| *byte == 0));
    Ok(())
}

#[test]
fn measurement_wraps_localized_text_before_clipping_and_releases_its_claim()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    let lease = MemoryLease::new(1024 * 1024, Box::new(Release));
    let short = MemoryLease::new(1, Box::new(Release));
    for locale in sugarsubstitute_contracts::localization::LOCALES {
        let mut owner = crate::text::FontOwner::new(locale.id)?;
        let message = sugarsubstitute_contracts::localization::installer::messages(locale.id)
            .language_description;
        let mut spec = spec();
        spec.layout.metrics = TextMetrics::Natural;
        spec.layout.extent = [90.0, 1.0];
        spec.layout.font_size = 14.0;
        spec.layout.line_height = 21.0;
        assert!(owner.measure(message, spec, Some(&short)).is_err());
        let narrow = owner.measure(message, spec, Some(&lease))?;
        assert!(narrow.lines > 1 && narrow.height > spec.layout.extent[1]);
        assert!((narrow.height - narrow.lines as f32 * 21.0).abs() < 0.05);
        assert!(lease.claim(1024 * 1024).is_ok());
        spec.layout.extent[0] = 1000.0;
        let wide = owner.measure(message, spec, Some(&lease))?;
        assert_eq!(wide.lines, 1);
        // Advance width includes trailing whitespace beyond the wrapping edge.
        assert!(narrow.width > 0.0 && wide.width > narrow.width);
        assert_eq!(owner.measure("", spec, Some(&lease))?, TextSize::default());
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn portable_measurement_uses_all_lines_and_logical_scale() -> Result<(), Box<dyn Error>> {
    let mut owner = crate::text::portable_probe::FontOwner::new("en")?;
    let mut spec = spec();
    spec.layout.extent = [150.0, 1.0];
    spec.layout.line_height = 40.0;
    let first = owner.measure("Hello\nHello\nHello", spec, None)?;
    spec.surface.scale = 2.0;
    let scaled = owner.measure("Hello\nHello\nHello", spec, None)?;
    assert_eq!(first.lines, 3);
    assert_eq!(scaled.lines, first.lines);
    assert!((first.height - 120.0).abs() < 0.01);
    assert!((scaled.height - first.height).abs() < 0.01);
    assert!((scaled.width - first.width).abs() < 0.01);
    Ok(())
}
