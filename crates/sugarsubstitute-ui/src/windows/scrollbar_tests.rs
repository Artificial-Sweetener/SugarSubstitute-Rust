// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared range and appearance contracts for transcript and page scrollbars.
use super::*;
fn model() -> Model {
    Model {
        appearance: Appearance::Overlay,
        bounds: RECT { left: 0, top: 0, right: 12, bottom: 160 },
        scale: 1.0,
        minimum: 0,
        maximum: 900,
        page: 100,
        position: 0,
        dark: true,
        enabled: true,
        hover: 0,
        pressed: Part::None,
    }
}

#[test]
fn native_range_and_hover_endpoints_keep_stable_input_geometry() -> Result<(), NativeUiError> {
    let base = model();
    let resting = geometry(&base)?;
    let expanded = geometry(&Model { hover: 255, ..base })?;
    assert_eq!(resting.thumb, RECT { left: 0, top: 12, right: 12, bottom: 30 });
    assert_eq!(resting.paint_thumb.left, 9.0);
    assert_eq!(resting.paint_thumb.right, 11.0);
    assert_eq!(expanded.paint_thumb.left, 3.0);
    assert_eq!(expanded.paint_thumb.right, 9.0);
    assert_eq!(resting.thumb, expanded.thumb);
    assert_eq!(hit_test(&base, POINT { x: 0, y: 15 })?, Part::Thumb);
    assert_eq!(hit_test(&base, POINT { x: 5, y: 5 })?, Part::Decrement);
    assert_eq!(hit_test(&base, POINT { x: 5, y: 140 })?, Part::PageAfter);
    assert_eq!(drag_position(&base, -100, 0)?, 0);
    assert_eq!(drag_position(&base, 1000, 0)?, 900);
    Ok(())
}

#[test]
fn scaling_tiny_ranges_and_extreme_positions_are_bounded() -> Result<(), NativeUiError> {
    for scale in [0.5, 1.0, 1.75, 2.0] {
        let base = Model {
            bounds: RECT {
                left: 0,
                top: 0,
                right: (12.0 * scale) as i32,
                bottom: (160.0 * scale) as i32,
            },
            scale,
            ..model()
        };
        let first = geometry(&base)?;
        let last = geometry(&Model { position: 900, ..base })?;
        assert_eq!(first.thumb.top, first.track.top);
        assert_eq!(last.thumb.bottom, last.track.bottom);
    }
    let tiny = Model { bounds: RECT { left: 0, top: 0, right: 12, bottom: 8 }, ..model() };
    assert_eq!(drag_position(&tiny, i32::MAX, i32::MIN)?, 0);
    let huge = Model {
        minimum: i32::MIN,
        maximum: i32::MAX,
        position: i32::MIN,
        page: u32::MAX,
        ..model()
    };
    let thumb = geometry(&huge)?.thumb;
    assert_eq!(thumb.bottom - thumb.top, 18);
    assert!(geometry(&Model { scale: f64::NAN, ..model() }).is_err());
    assert!(geometry(&Model { minimum: 10, maximum: 0, ..model() }).is_err());
    Ok(())
}

/// Page gutters retain the measured installer treatment without altering transcript chrome.
#[test]
fn persistent_gutter_matches_baseline_and_uses_the_same_scroll_range() -> Result<(), NativeUiError>
{
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let base = Model {
            appearance: Appearance::Gutter,
            scale,
            bounds: RECT {
                left: 0,
                top: 0,
                right: (15.0 * scale).round() as i32,
                bottom: (160.0 * scale).round() as i32,
            },
            ..model()
        };
        let resting = geometry(&base)?;
        let hovered = geometry(&Model { hover: 255, ..base })?;
        assert_eq!(resting.thumb, hovered.thumb);
        assert_eq!(resting.paint_thumb.left, hovered.paint_thumb.left);
        assert_eq!(resting.paint_thumb.right, hovered.paint_thumb.right);
        assert_eq!(resting.paint_thumb.top, resting.thumb.top as f32 + (3.0 * scale) as f32);
        assert_eq!(resting.paint_thumb.bottom, resting.thumb.bottom as f32 - (3.0 * scale) as f32);
        assert!((resting.paint_thumb.left - (3.0 * scale) as f32).abs() < 0.01);
        assert_eq!(drag_position(&base, i32::MAX, 0)?, base.maximum);
        assert_eq!(drag_position(&base, i32::MIN, 0)?, base.minimum);
        let color = foreground(&base);
        assert_eq!(
            [color.r, color.g, color.b, color.a],
            [159.0 / 255.0, 159.0 / 255.0, 159.0 / 255.0, 1.0]
        );
    }
    Ok(())
}
