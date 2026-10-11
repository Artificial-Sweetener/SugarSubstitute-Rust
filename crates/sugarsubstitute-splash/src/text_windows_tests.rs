// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise actual Windows font ownership, bounds and stable native raster output.

use super::StatusText;
use crate::{LOCALES, Theme};
use tiny_skia::Pixmap;

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
