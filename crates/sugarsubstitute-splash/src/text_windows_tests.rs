// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise actual Windows font ownership, bounds and stable native raster output.

use super::FontOwner;
use crate::{LOCALES, Theme};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use tiny_skia::Pixmap;

/// Drive the actual offline cursor to completion for native raster comparisons.
fn paint(
    text: &mut FontOwner,
    target: &mut Pixmap,
    message: &str,
    theme: Theme,
    scale: f32,
) -> std::io::Result<()> {
    let mut cursor =
        text.prepare_status_offline(message, theme, scale, target.width(), target.height())?;
    let max =
        NonZeroUsize::new(65_536).ok_or_else(|| std::io::Error::other("Invalid test budget"))?;
    while !cursor.step(target, max)? {}
    Ok(())
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
            for theme in [Theme::Dark, Theme::Light] {
                let mut target = Pixmap::new(size, size).ok_or("Cannot allocate clip test")?;
                target.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                let before = target.data().to_vec();
                let mut unclipped = target.clone();
                let mut cursor =
                    text.prepare_status_offline(locale.loading, theme, scale, size, size)?;
                cursor.clip = [0, 0, size as i32, size as i32];
                while !cursor
                    .step(&mut unclipped, NonZeroUsize::new(65_536).ok_or("Invalid budget")?)?
                {
                }
                paint(&mut text, &mut target, locale.loading, theme, scale)?;
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
                    Pixmap::new(size, size).ok_or("Cannot allocate cursor fixture")?;
                expected.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                paint(&mut text, &mut expected, locale.loading, theme, scale)?;
                for budget in [1, 17, 65_536, usize::MAX] {
                    let lease =
                        MemoryLease::new(6 * 1024 * 1024, Box::new(Release(released.clone())));
                    let mut target =
                        Pixmap::new(size, size).ok_or("Cannot allocate cursor fixture")?;
                    target.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                    let mut cursor =
                        text.prepare_status(locale.loading, theme, scale, size, size, lease)?;
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
    assert!(text.prepare_status("Loading...", Theme::Dark, 1.0, 558, 558, insufficient).is_err());
    assert_eq!(released.load(Ordering::SeqCst), 1);
    let lease = MemoryLease::new(1024 * 1024, Box::new(Release(released.clone())));
    let mut cursor =
        text.prepare_status("Loading...", Theme::Dark, 1.0, 558, 558, lease.clone())?;
    assert!(lease.claim(1).is_err());
    let mut target = Pixmap::new(558, 558).ok_or("Cannot allocate cursor fixture")?;
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
    let mut cursor = text.prepare_status_offline("Loading...", Theme::Dark, 1.0, 558, 558)?;
    drop(text);
    let mut wrong = Pixmap::new(557, 558).ok_or("Cannot allocate cursor fixture")?;
    assert!(cursor.step(&mut wrong, NonZeroUsize::new(1).ok_or("Invalid budget")?).is_err());
    assert!(wrong.data().iter().all(|byte| *byte == 0));
    let mut target = Pixmap::new(558, 558).ok_or("Cannot allocate cursor fixture")?;
    while !cursor.step(&mut target, NonZeroUsize::new(17).ok_or("Invalid budget")?)? {}
    assert!(target.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
    Ok(())
}

/// Every release-enabled label uses actual native font discovery and produces coverage.
#[test]
fn supported_labels_render_with_native_fallback() -> Result<(), Box<dyn std::error::Error>> {
    for locale in LOCALES {
        let mut text = FontOwner::new(locale.id)?;
        let runs = text.shape(locale.loading)?;
        assert!(!runs.is_empty());
        assert!(runs.iter().all(|run| run.indices.iter().all(|glyph| *glyph != 0)));
        let mut target = Pixmap::new(1116, 1116).ok_or("Cannot allocate status test")?;
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
        let mut target = Pixmap::new(698, 698).ok_or("Cannot allocate status test")?;
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
    let mut target = Pixmap::new(558, 558).ok_or("Cannot allocate status test")?;
    for scale in [f32::NAN, f32::INFINITY, 0.0, -1.0, 4.01] {
        assert!(paint(&mut text, &mut target, "Loading...", Theme::Dark, scale).is_err());
    }
    assert!(paint(&mut text, &mut target, &"x".repeat(4097), Theme::Dark, 1.0).is_err());
    assert!(target.data().iter().all(|byte| *byte == 0));
    Ok(())
}
