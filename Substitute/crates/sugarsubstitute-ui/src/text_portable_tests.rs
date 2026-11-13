// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect portable library raster behavior, cooperative composition and real claims.

use super::*;
use crate::bitmap::Bitmap;
use cosmic_text::SwashCache;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::configuration::Theme;
use sugarsubstitute_contracts::execution::MemoryRelease;
use tiny_skia::{FillRule, Paint, PathBuilder, Rect, Transform};

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Exercise the preexisting library draw boundary as an independent raster reference.
fn reference(
    text: &FontOwner,
    target: &mut Bitmap,
    message: &str,
    theme: Theme,
    scale: f32,
) -> io::Result<()> {
    let mut fonts = cursor::fonts(&text.fonts)?;
    let mut buffer = Buffer::new(&mut fonts, Metrics::new(14.0 * scale, 16.0 * scale));
    buffer.set_size(Some(510.0 * scale), Some(36.0 * scale));
    buffer.set_text(message, &Attrs::new().family(Family::SansSerif), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut fonts, false);
    let origin_y =
        buffer.layout_runs().next().map_or(504.5 * scale, |run| 519.0 * scale - run.line_y);
    let color = if theme == Theme::Dark { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) };
    buffer.draw(&mut fonts, &mut SwashCache::new(), color, |x, y, width, height, color| {
        if let Some(rect) = Rect::from_xywh(
            24.0 * scale + x as f32,
            origin_y + y as f32,
            width as f32,
            height as f32,
        ) {
            let mut paint = Paint { anti_alias: false, ..Paint::default() };
            paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
            target.fill_path(
                &PathBuilder::from_rect(rect),
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    });
    Ok(())
}

/// Every locale/scale matches the established portable draw output across chunk budgets.
#[test]
fn portable_status_chunks_match_library_raster_and_release_claims() -> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    for locale in sugarsubstitute_contracts::localization::LOCALES {
        let mut text = FontOwner::new(locale.id)?;
        for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
            let size = (558.0 * scale).round() as u32;
            for theme in [Theme::Dark, Theme::Light] {
                let mut expected =
                    Bitmap::new(size, size).ok_or("Cannot allocate portable fixture")?;
                expected.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                reference(&text, &mut expected, locale.loading, theme, scale)?;
                for budget in [1, 31, 65_536, usize::MAX] {
                    let lease =
                        MemoryLease::new(6 * 1024 * 1024, Box::new(Release(released.clone())));
                    let mut cursor = text.prepare(
                        locale.loading,
                        crate::text::fixtures::status(theme, scale, size, size, false),
                        lease,
                    )?;
                    let mut actual =
                        Bitmap::new(size, size).ok_or("Cannot allocate portable fixture")?;
                    actual.fill(tiny_skia::Color::from_rgba8(44, 77, 99, 180));
                    let max = NonZeroUsize::new(budget).ok_or("Invalid budget")?;
                    while !cursor.step(&mut actual, max)? {}
                    assert_eq!(
                        actual.data(),
                        expected.data(),
                        "{} scale{scale} budget{budget}",
                        locale.id
                    );
                    assert_eq!(cursor.cached_images(), 0);
                }
            }
        }
    }
    assert_eq!(released.load(Ordering::SeqCst), 200);
    Ok(())
}

/// Invalid preparation, wrong shape and missing scratch cannot modify the destination.
#[test]
fn portable_status_rejects_input_and_scratch_atomically() -> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let mut text = FontOwner::new("en")?;
    for scale in [f32::NAN, f32::INFINITY, -1.0, 0.0, 4.01] {
        assert!(
            text.prepare_offline(
                "Loading...",
                crate::text::fixtures::status(Theme::Dark, scale, 558, 558, false)
            )
            .is_err()
        );
    }
    assert!(
        text.prepare_offline(
            &"x".repeat(4097),
            crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, false)
        )
        .is_err()
    );
    assert!(
        text.prepare_offline(
            "Loading...",
            crate::text::fixtures::status(Theme::Dark, 1.0, 4097, 558, false)
        )
        .is_err()
    );
    let lease = MemoryLease::new(1024 * 1024, Box::new(Release(released.clone())));
    let mut cursor = text.prepare(
        "Loading...",
        crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, false),
        lease.clone(),
    )?;
    let max = NonZeroUsize::new(1).ok_or("Invalid budget")?;
    let mut target = Bitmap::new(558, 558).ok_or("Cannot allocate portable fixture")?;
    assert!(cursor.step(&mut target, max).is_err());
    assert!(target.data().iter().all(|byte| *byte == 0));
    drop(cursor);
    assert!(lease.claim(1024 * 1024).is_ok());
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    let mut cursor = text.prepare_offline(
        "Loading...",
        crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, false),
    )?;
    let mut wrong = Bitmap::new(557, 558).ok_or("Cannot allocate portable fixture")?;
    assert!(cursor.step(&mut wrong, max).is_err());
    assert!(wrong.data().iter().all(|byte| *byte == 0));
    drop(text);
    while !cursor.step(&mut target, max)? {}
    assert!(target.data().iter().any(|byte| *byte != 0));
    Ok(())
}

/// Overlapping private font access is a typed error instead of a UI/worker wait.
#[test]
fn portable_status_font_access_never_waits() -> Result<(), Box<dyn Error>> {
    fn worker_owned<T: Send>() {}
    worker_owned::<GlyphPaint>();
    let mut text = FontOwner::new("en")?;
    let owner = text.fonts.clone();
    let held = cursor::fonts(&owner)?;
    let failure = text
        .prepare_offline(
            "Loading...",
            crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, false),
        )
        .err()
        .ok_or("Overlapping font access unexpectedly succeeded")?;
    assert!(failure.get_ref().is_some_and(|cause| cause.is::<cursor::FontAccessError>()));
    drop(held);
    assert!(
        text.prepare_offline(
            "Loading...",
            crate::text::fixtures::status(Theme::Dark, 1.0, 558, 558, false)
        )
        .is_ok()
    );
    Ok(())
}
