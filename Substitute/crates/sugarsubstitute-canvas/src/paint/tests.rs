// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dab placement, footprint, falloff, accumulation and selection constraint against values
//! computed from CuteCanvas's brush formulas.
use super::{dabs::Dab, *};
use crate::{CanvasError, tests::harness};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_image_ops::{Coverage, Extent, FULL_COVERAGE, ImageError, Region};

fn run(
    test: impl FnOnce(&MemoryLease) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(16 * 1024 * 1024, |context| {
        test(context.memory()).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

const HARD: DabStyle = DabStyle { hardness: 1.0, opacity: 1.0, erase: false };

fn full(fraction: f64) -> f64 {
    fraction * f64::from(FULL_COVERAGE)
}

/// CuteCanvas's dab coverage at a pixel centre: opacity inside `hardness` of the radius, then a
/// linear fall to zero at the rim; nothing on or outside the circle.
fn expected(center: [f64; 2], diameter: f64, style: DabStyle, x: u32, y: u32) -> f64 {
    let radius = (diameter / 2.0).max(0.5);
    let [dx, dy] = [f64::from(x) + 0.5 - center[0], f64::from(y) + 0.5 - center[1]];
    if dx * dx + dy * dy >= radius * radius {
        return 0.0;
    }
    let t = dx.hypot(dy) / radius;
    let strength = if style.hardness >= 1.0 || t <= style.hardness {
        1.0
    } else {
        (1.0 - t) / (1.0 - style.hardness)
    };
    full(style.opacity * strength)
}

fn blank(memory: &MemoryLease, width: u32, height: u32) -> Result<Coverage, CanvasError> {
    Ok(Coverage::new(Extent::new(width, height)?, 0, memory)?)
}

#[test]
fn segments_space_dabs_at_a_fifth_of_the_smaller_diameter_and_interpolate_size() {
    let start = Contact::new([0.0, 0.0], 20.0);
    let end = Contact::new([10.0, 0.0], 20.0);
    let first: Vec<Dab> = segment(start, end, false).collect();
    let xs: Vec<f64> = first.iter().map(|dab| dab.center[0]).collect();
    assert_eq!(xs.len(), 4, "ceil(10 / 4) steps plus the start dab");
    for (actual, expected) in xs.iter().zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0]) {
        assert!((actual - expected).abs() < 1e-12);
    }
    let continued: Vec<Dab> = segment(start, end, true).collect();
    assert_eq!(continued, first[1..]);

    let tapered: Vec<f64> =
        segment(Contact::new([0.0, 0.0], 10.0), Contact::new([0.0, 6.0], 30.0), false)
            .map(|dab| dab.diameter)
            .collect();
    for (actual, expected) in tapered.iter().zip([10.0, 50.0 / 3.0, 70.0 / 3.0, 30.0]) {
        assert!((actual - expected).abs() < 1e-12);
    }
    let fine = segment(Contact::new([0.0, 0.0], 1.0), Contact::new([2.0, 0.0], 1.0), true);
    assert_eq!(fine.count(), 4, "spacing never falls below half a pixel");
    let resized = segment(start, Contact::new([0.0, 0.0], 30.0), true).collect::<Vec<_>>();
    assert_eq!(resized, [Dab { center: [0.0, 0.0], diameter: 20.0 }]);
}

#[test]
fn pen_pressure_scales_the_diameter_down_to_fifteen_percent() {
    assert!((pressure_diameter(100.0, 0.0) - 15.0).abs() < 1e-12);
    assert!((pressure_diameter(100.0, 0.5) - 57.5).abs() < 1e-12);
    assert!((pressure_diameter(100.0, 1.0) - 100.0).abs() < 1e-12);
    assert!((pressure_diameter(100.0, 7.0) - 100.0).abs() < 1e-12);
    assert_eq!(Contact::new([0.0; 2], 0.2).diameter, 1.0);
}

#[test]
fn hard_and_soft_dabs_cover_pixel_centres_inside_the_circle_with_cutecanvas_falloff()
-> harness::TestResult {
    run(|memory| {
        for (center, diameter, style) in [
            ([10.0, 10.0], 4.0, HARD),
            ([10.3, 9.6], 7.0, HARD),
            ([16.0, 16.0], 20.0, DabStyle { hardness: 0.5, opacity: 1.0, erase: false }),
            ([16.5, 15.25], 25.0, DabStyle { hardness: 0.0, opacity: 0.6, erase: false }),
            ([3.5, 3.5], 1.0, DabStyle { hardness: 0.2, opacity: 0.8, erase: false }),
        ] {
            let mut stroke = StrokeRaster::new(blank(memory, 40, 40)?, None, memory)?;
            stroke.stamp(Dab { center, diameter }, style)?;
            let coverage = stroke.flush()?;
            for y in 0..40 {
                for x in 0..40 {
                    let want = expected(center, diameter, style, x, y);
                    let got = f64::from(coverage.value(x, y));
                    assert!((got - want).abs() <= 1.0, "{center:?} {diameter} ({x},{y})");
                }
            }
        }
        Ok(())
    })
}

#[test]
fn overlapping_dabs_build_up_opacity_and_erasing_removes_it() -> harness::TestResult {
    run(|memory| {
        let half = DabStyle { hardness: 1.0, opacity: 0.5, erase: false };
        let dab = Dab { center: [8.0, 8.0], diameter: 6.0 };
        let mut stroke = StrokeRaster::new(blank(memory, 16, 16)?, None, memory)?;
        stroke.stamp(dab, half)?;
        stroke.stamp(dab, half)?;
        assert!((f64::from(stroke.flush()?.value(8, 8)) - full(0.75)).abs() <= 1.0);

        let covered = Coverage::new(Extent::new(16, 16)?, FULL_COVERAGE, memory)?;
        let mut eraser = StrokeRaster::new(covered, None, memory)?;
        eraser.stamp(dab, DabStyle { opacity: 0.25, erase: true, ..half })?;
        let erased = eraser.flush()?;
        assert!((f64::from(erased.value(8, 8)) - full(0.75)).abs() <= 1.0);
        assert_eq!(erased.value(0, 0), FULL_COVERAGE);
        eraser.stamp(dab, DabStyle { opacity: 1.0, erase: true, ..half })?;
        assert_eq!(eraser.flush()?.value(8, 8), 0);
        Ok(())
    })
}

#[test]
fn a_selection_constrains_the_finished_stroke_once() -> harness::TestResult {
    run(|memory| {
        let extent = Extent::new(32, 16)?;
        let mut selection = Coverage::new(extent, 0, memory)?;
        selection.update(Region::new(0, 0, 10, 16)?, memory, |_, _, _| FULL_COVERAGE)?;
        selection.update(Region::new(10, 0, 4, 16)?, memory, |_, _, _| FULL_COVERAGE / 2)?;
        let mut stroke = StrokeRaster::new(blank(memory, 32, 16)?, Some(selection), memory)?;
        for x in [6.0, 9.0, 12.0, 15.0, 18.0] {
            stroke.stamp(Dab { center: [x, 8.0], diameter: 12.0 }, HARD)?;
        }
        let coverage = stroke.flush()?;
        assert_eq!(coverage.value(5, 8), FULL_COVERAGE);
        assert_eq!(coverage.value(12, 8), FULL_COVERAGE / 2, "blended once, not per dab");
        assert_eq!(coverage.value(16, 8), 0, "outside the selection");
        assert!(
            StrokeRaster::new(blank(memory, 8, 8)?, Some(blank(memory, 9, 8)?), memory).is_err()
        );
        Ok(())
    })
}

#[test]
fn flushing_publishes_only_painted_tiles_and_ignores_dabs_off_the_mask() -> harness::TestResult {
    run(|memory| {
        let base = blank(memory, 1000, 600)?;
        let mut stroke = StrokeRaster::new(base.clone(), None, memory)?;
        stroke.stamp(Dab { center: [-500.0, 10.0], diameter: 50.0 }, HARD)?;
        stroke.stamp(Dab { center: [10.0, 1e12], diameter: 50.0 }, HARD)?;
        assert_eq!(stroke.flush()?.difference(&base), None);
        stroke.stamp(Dab { center: [300.0, 300.0], diameter: 20.0 }, HARD)?;
        let first = stroke.flush()?.clone();
        assert_eq!(first.difference(&base), Some(Region::new(256, 256, 128, 128)?));
        stroke.stamp(Dab { center: [700.0, 64.0], diameter: 10.0 }, HARD)?;
        let second = stroke.flush()?;
        assert_eq!(second.difference(&first), Some(Region::new(640, 0, 128, 128)?));
        assert_eq!(second.value(300, 300), FULL_COVERAGE);
        let invalid = Dab { center: [f64::NAN, 0.0], diameter: 4.0 };
        assert!(stroke.stamp(invalid, HARD).is_err());
        Ok(())
    })
}
