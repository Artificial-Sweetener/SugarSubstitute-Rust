// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve observed coverage and reject invalid progress inputs.
use super::*;

/// A widget can move and resize without retaining any splash-specific coordinates.
#[test]
fn progress_geometry_translates_exactly_at_multiple_scales()
-> Result<(), Box<dyn std::error::Error>> {
    for appearance in [
        ProgressAppearance::Hairline { dark: true },
        ProgressAppearance::Panel { dark: false },
        ProgressAppearance::Wizard,
    ] {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let bar = ProgressBar { x: 4.0, y: 8.0, width: 48.0, height: 4.0, appearance };
            let shifted = ProgressBar { x: 12.0, y: 16.0, ..bar };
            let frame = ProgressFrame { completed: 0.4, activity: Some(0.5) };
            let mut left = Bitmap::new(128, 128).ok_or("Missing test raster")?;
            let mut right = left.clone();
            for (geometry, target) in [(bar, &mut left), (shifted, &mut right)] {
                let mut cursor = ProgressCursor::new(128, 128, geometry, frame, scale)?;
                while !cursor.step(target)? {}
            }
            let offset = (8.0 * scale) as u32;
            for y in 0..128 - offset {
                for x in 0..128 - offset {
                    assert_eq!(left.pixel(x, y), right.pixel(x + offset, y + offset));
                }
            }
        }
    }
    Ok(())
}

/// Invalid progress and stale destination sizes fail before any pixel is changed.
#[test]
fn invalid_progress_and_destination_are_rejected_atomically()
-> Result<(), Box<dyn std::error::Error>> {
    let bar = ProgressBar {
        x: 4.0,
        y: 8.0,
        width: 48.0,
        height: 4.0,
        appearance: ProgressAppearance::Wizard,
    };
    let frame = ProgressFrame { completed: 0.5, activity: None };
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY, 4.1] {
        assert!(ProgressCursor::new(64, 32, bar, frame, scale).is_err());
    }
    for completed in [-0.1, 1.1, f32::NAN] {
        assert!(
            ProgressCursor::new(64, 32, bar, ProgressFrame { completed, ..frame }, 1.0).is_err()
        );
    }
    let mut cursor = ProgressCursor::new(64, 32, bar, frame, 1.0)?;
    let mut target = Bitmap::new(32, 16).ok_or("Missing test raster")?;
    let before = target.clone();
    assert!(cursor.step(&mut target).is_err());
    assert_eq!(target.data(), before.data());
    Ok(())
}

/// Keep installer accent and track colors under the same widget owner.
#[test]
fn wizard_progress_uses_its_existing_accent_and_track() -> Result<(), Box<dyn std::error::Error>> {
    let mut target = Bitmap::new(64, 32).ok_or("Missing test raster")?;
    let bar = ProgressBar {
        x: 4.0,
        y: 8.0,
        width: 48.0,
        height: 4.0,
        appearance: ProgressAppearance::Wizard,
    };
    let mut cursor =
        ProgressCursor::new(64, 32, bar, ProgressFrame { completed: 0.5, activity: None }, 1.0)?;
    while !cursor.step(&mut target)? {}
    let fill = target.pixel(10, 9).ok_or("Missing fill")?;
    assert_eq!((fill.red(), fill.green(), fill.blue(), fill.alpha()), (255, 64, 137, 255));
    assert_eq!(target.pixel(42, 9).map(|p| p.alpha()), Some(56));
    Ok(())
}
/// Reject invalid producer fractions without converting them into artificial progress.
#[test]
fn progress_inputs_require_finite_bounded_units() {
    assert!(ProgressFrame { completed: 0.4, activity: Some(0.5) }.is_valid());
    assert!(!ProgressFrame { completed: f32::NAN, activity: None }.is_valid());
    assert!(!ProgressFrame { completed: 0.0, activity: Some(2.0) }.is_valid());
}

/// Retain the actual native intersected silhouette's fractional bottom-corner area.
#[test]
fn expanded_clip_intersects_before_coverage() -> Result<(), Box<dyn std::error::Error>> {
    for (scale, x, y, expected) in
        [(1.25, 667, 452, 61), (1.75, 934, 633, 61), (2.0, 1066, 718, 156)]
    {
        let path = ProgressBar {
            x: 24.0,
            y: 356.0,
            width: 510.0,
            height: 6.0,
            appearance: ProgressAppearance::Panel { dark: true },
        }
        .clip(510.0, scale)
        .ok_or("Missing expanded clip")?;
        let mut raster = crate::shapes::CoverageRaster::new(&path, 1116, 1116, true)?;
        while !raster.step(8192) {}
        let mask = raster.finish()?;
        assert!(
            mask.data()[y * 1116 + x].abs_diff(expected) <= 1,
            "actual {} expected {expected}",
            mask.data()[y * 1116 + x]
        );
        if scale == 2.0 {
            assert_eq!(mask.data()[y * 1116 + x], expected);
        }
    }
    Ok(())
}
