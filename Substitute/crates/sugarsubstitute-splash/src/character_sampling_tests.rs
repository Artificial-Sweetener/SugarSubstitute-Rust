// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Qualify the native character sampler's interpolation, composition, and clipping.

use super::sample::{bounded_count, sample};
use super::{DeviceTransform, circle_above, multiply, over, paint, paper_edge};
use crate::projection::{Bounds, Point};
use std::error::Error;
use sugarsubstitute_ui::bitmap::Bitmap;

/// Preserve alpha validity and the source raster's downward interpolation at fractions.
#[test]
fn precise_sampling_retains_premultiplied_channels() -> Result<(), Box<dyn Error>> {
    let mut source = Bitmap::new(2, 2).ok_or("Cannot allocate sample raster")?;
    source.data_mut().copy_from_slice(&[
        101, 24, 18, 255, 102, 34, 27, 255, 102, 84, 81, 255, 114, 153, 148, 255,
    ]);
    assert_eq!(sample(&source, 16_383, 49_151, false), [103, 81, 77, 255]);
    assert_eq!(sample(&source, -32_768, -32_768, false), [101, 24, 18, 255]);
    assert_eq!(sample(&source, i64::MAX, i64::MAX, false), [114, 153, 148, 255]);
    Ok(())
}

/// Bound alpha multiplication and compose through actual pixel coverage.
#[test]
fn premultiplied_over_preserves_source_and_destination_limits() {
    assert_eq!(multiply(255, 255), 255);
    assert_eq!(multiply(255, 0), 0);
    assert_eq!(multiply(191, 253), 189);
    assert_eq!(multiply(226, 233), 206);
    let mut destination = [30, 40, 50, 255];
    over(&mut destination, [10, 20, 30, 128], 0);
    assert_eq!(destination, [30, 40, 50, 255]);
    over(&mut destination, [10, 20, 30, 128], 255);
    assert_eq!(destination, [25, 40, 55, 255]);
}

/// Preserve fixed-coordinate end bounds, including stationary and reversed sampling.
#[test]
fn complete_interpolation_steps_have_explicit_end_bounds() {
    assert_eq!(bounded_count(32_768, 65_536, 3), 1);
    assert_eq!(bounded_count(65_536, -65_536, 3), 1);
    assert_eq!(bounded_count(0, 0, 3), usize::MAX);
}

/// Retain symmetric quarter-pixel coverage and the original widget's physical clip.
#[test]
fn paper_edge_covers_fractional_columns_and_clips_widget_rows() -> Result<(), Box<dyn Error>> {
    let mut target = Bitmap::new(12, 12).ok_or("Cannot allocate edge raster")?;
    paper_edge(
        &mut target,
        Point { x: 5.0, y: 1.0 },
        Point { x: 5.0, y: 10.0 },
        2.5,
        89,
        Bounds { x: 0.0, y: 3.5, width: 12.0, height: 8.0 },
    );
    for x in [3, 6] {
        let offset = (6 * 12 + x) * 4;
        assert_eq!(&target.data()[offset..offset + 4], &[5, 2, 2, 22]);
    }
    assert_eq!(&target.data()[3 * 12 * 4..4 * 12 * 4], &[0; 48]);
    assert!((circle_above(-1.0, 1.0, 1.0, 0.0) - std::f64::consts::PI / 2.0).abs() < 1e-12);
    assert_eq!(circle_above(-1.0, 1.0, 1.0, 1.0), 0.0);
    Ok(())
}

/// Clip opaque airborne source pixels to the same widget rectangle as the paper edge.
#[test]
fn transformed_card_cannot_paint_outside_its_widget() -> Result<(), Box<dyn Error>> {
    let mut source = Bitmap::new(4, 4).ok_or("Cannot allocate source raster")?;
    source.fill(tiny_skia::Color::WHITE);
    let quad = [
        Point { x: 3.0, y: 1.0 },
        Point { x: 7.0, y: 0.0 },
        Point { x: 7.0, y: 8.0 },
        Point { x: 3.0, y: 9.0 },
    ];
    let mut target = Bitmap::new(12, 12).ok_or("Cannot allocate target raster")?;
    let mapping = DeviceTransform::from_quad(quad, 4.0, Point { x: 0.0, y: 0.0 }, 1.0);
    paint(&mut target, &source, quad, Bounds { x: 2.0, y: 3.0, width: 6.0, height: 4.0 }, mapping)?;
    assert!(target.pixel(5, 4).is_some_and(|pixel| pixel.alpha() == 255));
    for y in 0..12 {
        for x in 0..12 {
            if !(2..8).contains(&x) || !(3..7).contains(&y) {
                assert_eq!(target.pixel(x, y).map(|pixel| pixel.alpha()), Some(0));
            }
        }
    }
    paint(
        &mut target,
        &source,
        quad,
        Bounds { x: 20.0, y: 20.0, width: 1.0, height: 1.0 },
        mapping,
    )?;
    Ok(())
}

/// Match the observed Qt widget clip at quarter- and three-quarter-pixel DPI origins.
#[test]
fn fractional_widget_origins_snap_once_to_device_pixels() -> Result<(), Box<dyn Error>> {
    for (origin, first_row) in [(8.75, 9), (12.25, 12)] {
        let mut target = Bitmap::new(20, 20).ok_or("Cannot allocate edge raster")?;
        paper_edge(
            &mut target,
            Point { x: 10.0, y: 1.0 },
            Point { x: 10.0, y: 18.0 },
            2.0,
            89,
            Bounds { x: 0.0, y: origin, width: 20.0, height: 20.0 },
        );
        assert_eq!(target.pixel(10, first_row - 1).map(|pixel| pixel.alpha()), Some(0));
        assert_eq!(target.pixel(10, first_row).map(|pixel| pixel.alpha()), Some(89));
    }
    Ok(())
}

/// Preserve the callback span count across eight-row sampling slices and its 256-span flush.
#[test]
fn sampling_yields_keep_cross_row_native_span_history() -> Result<(), Box<dyn Error>> {
    let mut source = Bitmap::new(4, 4).ok_or("Cannot allocate bounded source")?;
    source.fill(tiny_skia::Color::WHITE);
    let quad = [
        Point { x: 3.0, y: 0.0 },
        Point { x: 40.0, y: 3.0 },
        Point { x: 40.0, y: 317.0 },
        Point { x: 3.0, y: 320.0 },
    ];
    let mapping = DeviceTransform::from_quad(quad, 4.0, Point { x: 0.0, y: 0.0 }, 1.0);
    let mut target = Bitmap::new(64, 320).ok_or("Cannot allocate bounded target")?;
    let mut cursor = super::SamplingCursor::new(
        64,
        320,
        quad,
        Bounds { x: 0.0, y: 0.0, width: 64.0, height: 320.0 },
        mapping,
    )?;
    let mut retained = false;
    let mut flushed = false;
    loop {
        let previous_row = cursor.row;
        let previous_spans = cursor.span_count;
        let done = cursor.step(&mut target, &source)?;
        assert!(cursor.row - previous_row <= 8);
        retained |= cursor.span_count > 100;
        flushed |= cursor.span_count < previous_spans;
        if done {
            break;
        }
    }
    assert!(retained && flushed);
    Ok(())
}
