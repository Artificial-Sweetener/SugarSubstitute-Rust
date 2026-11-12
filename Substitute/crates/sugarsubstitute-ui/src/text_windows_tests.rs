// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise actual Windows font ownership, bounds and stable native raster output.

use super::FontOwner;
use crate::bitmap::Bitmap;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use sugarsubstitute_contracts::{configuration::Theme, localization::LOCALES};

/// Natural typography keeps fractional origins; grid-fitted labels retain their
/// established device-pixel placement even when logical input moves within a pixel.
#[test]
fn natural_metrics_preserve_fractional_glyph_positions() -> Result<(), Box<dyn std::error::Error>> {
    use crate::text::TextMetrics;
    let mut owner = FontOwner::new("en")?;
    for metrics in [TextMetrics::GdiCompatible, TextMetrics::Natural] {
        let mut images = Vec::new();
        for offset in [0.0, 0.25] {
            let mut spec = crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, true);
            spec.layout.metrics = metrics;
            spec.layout.origin[0] += offset;
            let mut target = Bitmap::new(558, 558).ok_or("Target absent")?;
            let mut cursor = owner.prepare_offline("Choose your language", spec)?;
            while !cursor.step(&mut target, NonZeroUsize::new(32768).ok_or("Budget absent")?)? {}
            images.push(target);
        }
        assert_eq!(images[0].data() == images[1].data(), metrics == TextMetrics::GdiCompatible);
    }
    Ok(())
}

/// Optional native arrays use font advances and zero offsets; callback errors remain observable.
#[test]
fn native_glyph_runs_accept_optional_arrays_and_retain_callback_errors()
-> Result<(), Box<dyn std::error::Error>> {
    use std::{cell::RefCell, mem::ManuallyDrop};
    use windows::{Win32::Graphics::DirectWrite::*, core::ComObject};
    let text = FontOwner::new("en")?;
    let shaped =
        text.shape("Loading...", crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, true))?;
    let source = shaped.first().ok_or("Missing native glyphs")?;
    let collector = ComObject::new(super::Collector {
        runs: RefCell::new(Vec::with_capacity(1)),
        failure: RefCell::new(None),
        colors: Vec::new(),
    });
    let renderer: IDWriteTextRenderer = collector.to_interface();
    let mut run = DWRITE_GLYPH_RUN {
        fontFace: ManuallyDrop::new(Some(source.face.clone())),
        fontEmSize: source.size,
        glyphCount: source.indices.len() as u32,
        glyphIndices: source.indices.as_ptr(),
        ..Default::default()
    };
    // SAFETY: The retained font and glyph slice remain live through synchronous callbacks.
    let result = unsafe {
        renderer.DrawGlyphRun(
            None,
            0.0,
            0.0,
            DWRITE_MEASURING_MODE_NATURAL,
            &run,
            std::ptr::null(),
            None,
        )
    };
    // SAFETY: This fixture owns the sole manually retained reference in the native struct.
    unsafe { ManuallyDrop::drop(&mut run.fontFace) };
    result?;
    assert!(collector.get().failure.borrow().is_none());
    let copied = collector.get().runs.borrow();
    assert_eq!(copied[0].indices, source.indices);
    assert!(copied[0].advances.iter().all(|advance| advance.is_finite() && *advance > 0.0));
    assert!(
        copied[0]
            .offsets
            .iter()
            .all(|offset| offset.advanceOffset == 0.0 && offset.ascenderOffset == 0.0)
    );
    drop(copied);
    assert!(
        // SAFETY: Null input exercises callback validation before dereference.
        unsafe {
            renderer.DrawGlyphRun(
                None,
                0.0,
                0.0,
                DWRITE_MEASURING_MODE_NATURAL,
                std::ptr::null(),
                std::ptr::null(),
                None,
            )
        }
        .is_err()
    );
    assert!(collector.get().failure.borrow().is_some());
    Ok(())
}

/// Drive the actual offline cursor to completion for native raster comparisons.
fn paint(
    text: &mut FontOwner,
    target: &mut Bitmap,
    message: &str,
    theme: Theme,
    scale: f32,
) -> std::io::Result<()> {
    let mut cursor = text.prepare_offline(
        message,
        crate::text::fixtures::status(theme, scale, target.width(), target.height(), true),
    )?;
    let max =
        NonZeroUsize::new(65_536).ok_or_else(|| std::io::Error::other("Invalid test budget"))?;
    while !cursor.step(target, max)? {}
    Ok(())
}

/// Compare one row span of RGBA pixels starting at device column `x`.
/// Whole-span byte equality keeps unoptimized test builds fast; a mismatch reports its first pixel.
fn assert_pixels(actual: &[u8], expected: &[u8], x: u32, y: u32, label: &str) {
    if actual == expected {
        return;
    }
    assert_eq!(actual.len(), expected.len(), "{label} row {y} span lengths");
    for (offset, (actual, expected)) in
        actual.chunks_exact(4).zip(expected.chunks_exact(4)).enumerate()
    {
        assert_eq!(actual, expected, "{label} at ({},{y})", x + offset as u32);
    }
}

/// Clip every label over prepainted pixels without changing interior native glyph coverage.
#[test]
fn all_labels_preserve_pixels_outside_status_widget() -> Result<(), Box<dyn std::error::Error>> {
    for locale in LOCALES {
        let mut text = FontOwner::new(locale.id)?;
        for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
            let size = (558.0 * scale).round() as u32;
            let bounds = [
                (24.0 * scale).round() as u32,
                (486.0 * scale).round() as u32,
                (534.0 * scale).round() as u32,
                (530.0 * scale).round() as u32,
            ];
            for message in [locale.loading, locale.loading_long, locale.loading_extended]
                .into_iter()
                .chain(locale.startup.labels())
            {
                for theme in [Theme::Dark, Theme::Light] {
                    let mut target = Bitmap::new(size, size).ok_or("Cannot allocate clip test")?;
                    target.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                    let before = target.data().to_vec();
                    let mut unclipped = target.clone();
                    let mut cursor = text.prepare_offline(
                        message,
                        crate::text::fixtures::status(theme, scale, size, size, true),
                    )?;
                    cursor.clip = [0, 0, size as i32, size as i32];
                    while !cursor
                        .step(&mut unclipped, NonZeroUsize::new(65_536).ok_or("Invalid budget")?)?
                    {
                    }
                    paint(&mut text, &mut target, message, theme, scale)?;
                    let mut visible = false;
                    let label = format!("{} {theme:?} scale {scale} for {message:?}", locale.id);
                    let row_bytes = size as usize * 4;
                    let [left, top, right, bottom] = bounds;
                    let interior = left as usize * 4..right as usize * 4;
                    for (y, ((actual, original), expected)) in target
                        .data()
                        .chunks_exact(row_bytes)
                        .zip(before.chunks_exact(row_bytes))
                        .zip(unclipped.data().chunks_exact(row_bytes))
                        .enumerate()
                    {
                        let y = y as u32;
                        if y < top || y >= bottom {
                            assert_pixels(actual, original, 0, y, &label);
                            continue;
                        }
                        assert_pixels(
                            &actual[..interior.start],
                            &original[..interior.start],
                            0,
                            y,
                            &label,
                        );
                        assert_pixels(
                            &actual[interior.end..],
                            &original[interior.end..],
                            right,
                            y,
                            &label,
                        );
                        assert_pixels(
                            &actual[interior.clone()],
                            &expected[interior.clone()],
                            left,
                            y,
                            &label,
                        );
                        visible |= actual[interior.clone()] != original[interior.clone()];
                    }
                    assert!(
                        visible,
                        "{} scale {scale} has no interior glyph coverage for {message:?}",
                        locale.id
                    );
                }
            }
        }
    }
    Ok(())
}

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Real admitted cursors preserve every raster byte across cooperative budgets.
#[test]
fn status_cursor_chunk_budgets_preserve_exact_raster() -> Result<(), Box<dyn std::error::Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    for locale in LOCALES {
        let mut text = FontOwner::new(locale.id)?;
        for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
            let size = (558.0 * scale).round() as u32;
            for theme in [Theme::Dark, Theme::Light] {
                let mut expected =
                    Bitmap::new(size, size).ok_or("Cannot allocate cursor fixture")?;
                expected.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                paint(&mut text, &mut expected, locale.loading, theme, scale)?;
                for budget in [1, 17, 65_536, usize::MAX] {
                    let lease =
                        MemoryLease::new(6 * 1024 * 1024, Box::new(Release(released.clone())));
                    let mut target =
                        Bitmap::new(size, size).ok_or("Cannot allocate cursor fixture")?;
                    target.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                    let mut cursor = text.prepare(
                        locale.loading,
                        crate::text::fixtures::status(theme, scale, size, size, true),
                        lease,
                    )?;
                    // Font owner teardown cannot invalidate the cursor's retained native faces.
                    let max = NonZeroUsize::new(budget).ok_or("Invalid budget")?;
                    let mut steps = 0;
                    while !cursor.step(&mut target, max)? {
                        steps += 1;
                    }
                    assert!(steps > 0);
                    assert_eq!(
                        target.data(),
                        expected.data(),
                        "{} scale{scale} budget{budget}",
                        locale.id
                    );
                }
            }
        }
    }
    assert_eq!(released.load(Ordering::SeqCst), 200);
    Ok(())
}

/// Metadata/texture admission fails before mutation and releases claims on cursor Drop.
#[test]
fn status_cursor_rejects_undercharged_scratch_without_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let mut text = FontOwner::new("en")?;
    let insufficient = MemoryLease::new(1, Box::new(Release(released.clone())));
    assert!(
        text.prepare(
            "Loading...",
            crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, true),
            insufficient
        )
        .is_err()
    );
    assert_eq!(released.load(Ordering::SeqCst), 1);
    let lease = MemoryLease::new(1024 * 1024, Box::new(Release(released.clone())));
    let mut cursor = text.prepare(
        "Loading...",
        crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, true),
        lease.clone(),
    )?;
    assert!(lease.claim(1).is_err());
    let mut target = Bitmap::new(558, 558).ok_or("Cannot allocate cursor fixture")?;
    assert!(cursor.step(&mut target, NonZeroUsize::new(1).ok_or("Invalid budget")?).is_err());
    assert!(target.data().iter().all(|byte| *byte == 0));
    drop(cursor);
    assert!(lease.claim(1024 * 1024).is_ok());
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 2);
    Ok(())
}

/// A cursor owns all native resources after its font owner drops; wrong shape is atomic.
#[test]
fn status_cursor_owns_native_resources_and_rejects_changed_target()
-> Result<(), Box<dyn std::error::Error>> {
    let mut text = FontOwner::new("en")?;
    let mut cursor = text.prepare_offline(
        "Loading...",
        crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, true),
    )?;
    drop(text);
    let mut wrong = Bitmap::new(557, 558).ok_or("Cannot allocate cursor fixture")?;
    assert!(cursor.step(&mut wrong, NonZeroUsize::new(1).ok_or("Invalid budget")?).is_err());
    assert!(wrong.data().iter().all(|byte| *byte == 0));
    let mut target = Bitmap::new(558, 558).ok_or("Cannot allocate cursor fixture")?;
    while !cursor.step(&mut target, NonZeroUsize::new(17).ok_or("Invalid budget")?)? {}
    assert!(target.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
    Ok(())
}

/// Every release-enabled label uses actual native font discovery and produces coverage.
#[test]
fn supported_labels_render_with_native_fallback() -> Result<(), Box<dyn std::error::Error>> {
    for locale in LOCALES {
        let mut text = FontOwner::new(locale.id)?;
        let runs = text.shape(
            locale.loading,
            crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, true),
        )?;
        assert!(!runs.is_empty());
        assert!(runs.iter().all(|run| run.indices.iter().all(|glyph| *glyph != 0)));
        let mut target = Bitmap::new(1116, 1116).ok_or("Cannot allocate status test")?;
        paint(&mut text, &mut target, locale.loading, Theme::Dark, 2.0)?;
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
        let mut text = FontOwner::new("en")?;
        let mut target = Bitmap::new(698, 698).ok_or("Cannot allocate status test")?;
        paint(&mut text, &mut target, "Loading...", Theme::Dark, 1.25)?;
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
    assert!(FontOwner::new(&"x".repeat(65)).is_err());
    assert!(FontOwner::new("en\0ja").is_err());
    let mut text = FontOwner::new("en")?;
    let mut target = Bitmap::new(558, 558).ok_or("Cannot allocate status test")?;
    for scale in [f32::NAN, f32::INFINITY, 0.0, -1.0, 4.01] {
        assert!(paint(&mut text, &mut target, "Loading...", Theme::Dark, scale).is_err());
    }
    assert!(paint(&mut text, &mut target, &"x".repeat(4097), Theme::Dark, 1.0).is_err());
    assert!(target.data().iter().all(|byte| *byte == 0));
    Ok(())
}
