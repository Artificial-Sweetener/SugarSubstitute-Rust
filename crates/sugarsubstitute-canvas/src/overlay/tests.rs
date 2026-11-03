// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Contour tracing fixtures, outline caching and ant animation.
use super::*;
use crate::{CanvasError, tests::harness};
use sugarsubstitute_image_ops::{CombineMode, Coverage, FULL_COVERAGE, ImageError, Path, Region};

fn trace(
    coverage: &Coverage,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<CoverageContours, CanvasError> {
    CoverageContours::trace(coverage, context)
}

/// Twice the signed area of a contour; positive is clockwise on screen (y down).
fn area(contour: &Contour) -> f64 {
    let points = contour.points();
    (0..points.len())
        .map(|index| {
            let [x0, y0] = points[index].map(f64::from);
            let [x1, y1] = points[(index + 1) % points.len()].map(f64::from);
            x0 * y1 - x1 * y0
        })
        .sum::<f64>()
        / 2.0
}

fn bounds(contour: &Contour) -> [f32; 4] {
    contour
        .points()
        .iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |[l, t, r, b], [x, y]| {
            [l.min(*x), t.min(*y), r.max(*x), b.max(*y)]
        })
}

fn run(
    test: impl FnOnce(&sugarsubstitute_contracts::execution::JobContext) -> Result<(), CanvasError>
    + Send
    + 'static,
) -> harness::TestResult {
    harness::run(16 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

#[test]
fn hard_edges_trace_pixel_edges_with_half_pixel_corner_cuts() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(300, 200)?;
        let square = Path::rectangle([10.0, 20.0, 50.0, 70.0])?.rasterize(extent, context)?;
        let contours = trace(&square, context)?;
        assert_eq!(contours.contours().len(), 1);
        let contour = &contours.contours()[0];
        assert_eq!(bounds(contour), [10.0, 20.0, 50.0, 70.0]);
        // Corner cells cut each corner by a half-pixel triangle of area 1/8.
        assert!((area(contour) - (40.0 * 50.0 - 4.0 / 8.0)).abs() < 1e-3);
        assert_eq!(contours.fingerprint(), square.fingerprint());
        // Coverage touching the extent edge still closes along it.
        let edge = Path::rectangle([0.0, 0.0, 300.0, 40.0])?.rasterize(extent, context)?;
        let traced = trace(&edge, context)?;
        assert_eq!(bounds(&traced.contours()[0]), [0.0, 0.0, 300.0, 40.0]);
        let everything = Coverage::new(extent, FULL_COVERAGE, context.memory())?;
        assert_eq!(bounds(&trace(&everything, context)?.contours()[0]), [0.0, 0.0, 300.0, 200.0]);
        assert!(trace(&Coverage::new(extent, 0, context.memory())?, context)?.is_empty());
        Ok(())
    })
}

#[test]
fn holes_and_islands_trace_separately_with_opposite_winding() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(256, 256)?;
        let outer = Path::rectangle([20.0, 20.0, 120.0, 120.0])?.rasterize(extent, context)?;
        let hole = Path::rectangle([50.0, 50.0, 80.0, 80.0])?.rasterize(extent, context)?;
        let island = Path::ellipse([150.0, 140.0, 230.0, 200.0])?.rasterize(extent, context)?;
        let ring = outer.combine(&hole, CombineMode::Subtract, context)?;
        let shapes = ring.combine(&island, CombineMode::Add, context)?;
        let contours = trace(&shapes, context)?;
        assert_eq!(contours.contours().len(), 3);
        let mut found: Vec<([f32; 4], bool)> = contours
            .contours()
            .iter()
            .map(|contour| (bounds(contour), area(contour) > 0.0))
            .collect();
        found.sort_by(|left, right| left.0[0].total_cmp(&right.0[0]));
        assert_eq!(found[0], ([20.0, 20.0, 120.0, 120.0], true));
        assert_eq!(found[1], ([50.0, 50.0, 80.0, 80.0], false));
        let ellipse = found[2];
        assert!(ellipse.1);
        for (actual, expected) in ellipse.0.iter().zip([150.0, 140.0, 230.0, 200.0]) {
            assert!((actual - expected).abs() < 0.5, "{actual} vs {expected}");
        }
        Ok(())
    })
}

#[test]
fn partial_coverage_traces_its_half_level_crossing() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(64, 64)?;
        let mut coverage = Coverage::new(extent, 0, context.memory())?;
        // A horizontal ramp: column x holds x/63 of full coverage inside rows 10..30.
        coverage.update(Region::new(0, 10, 64, 20)?, context.memory(), |x, _, _| {
            (u32::from(FULL_COVERAGE) * x / 63) as u16
        })?;
        let contours = trace(&coverage, context)?;
        assert_eq!(contours.contours().len(), 1);
        let [left, top, right, bottom] = bounds(&contours.contours()[0]);
        // Half coverage falls between columns 31 and 32, whose centres are 31.5 and 32.5.
        assert!((left - 32.0).abs() < 0.01, "{left}");
        assert_eq!([top, right, bottom], [10.0, 64.0, 30.0]);
        // A pixel just below half is outside; just above is inside.
        let mut dot = Coverage::new(extent, 0, context.memory())?;
        dot.write(
            Region::new(5, 5, 2, 1)?,
            &[FULL_COVERAGE / 2, FULL_COVERAGE / 2 + 1],
            context.memory(),
        )?;
        let traced = trace(&dot, context)?;
        assert_eq!(traced.contours().len(), 1);
        let [l, _, r, _] = bounds(&traced.contours()[0]);
        assert!((l - 6.0).abs() < 1e-3 && r < 6.6, "{l} {r}");
        Ok(())
    })
}

#[test]
fn saddles_join_diagonal_pixels_by_their_mean() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(8, 8)?;
        let mut coverage = Coverage::new(extent, 0, context.memory())?;
        coverage.write(
            Region::new(2, 2, 2, 2)?,
            &[FULL_COVERAGE, 0, 0, FULL_COVERAGE],
            context.memory(),
        )?;
        // The saddle's mean is exactly half, below the inside level: two separate pixels.
        assert_eq!(trace(&coverage, context)?.contours().len(), 2);
        // A third corner still below half lifts the mean above it and joins the diagonal.
        coverage.write(
            Region::new(2, 2, 2, 2)?,
            &[FULL_COVERAGE, 30_000, 0, FULL_COVERAGE],
            context.memory(),
        )?;
        assert_eq!(trace(&coverage, context)?.contours().len(), 1);
        Ok(())
    })
}

#[test]
fn outlines_are_traced_once_per_selection_and_cached() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(128, 128)?;
        let first = Path::rectangle([1.0, 1.0, 9.0, 9.0])?.rasterize(extent, context)?;
        let second = Path::rectangle([3.0, 1.0, 9.0, 9.0])?.rasterize(extent, context)?;
        let mut outlines = SelectionOutlines::default();
        assert!(outlines.request(None).is_none());
        let work = outlines.request(Some(&first)).ok_or(CanvasError::Invalid("No work"))?;
        assert_eq!(work.fingerprint(), first.fingerprint());
        assert!(outlines.request(Some(&first)).is_none());
        assert!(outlines.get(Some(&first)).is_none());
        outlines.complete(work.run(context)?);
        assert!(outlines.get(Some(&first)).is_some());
        assert!(outlines.request(Some(&first)).is_none());
        let work = outlines.request(Some(&second)).ok_or(CanvasError::Invalid("No work"))?;
        outlines.abandon(work.fingerprint());
        let retry = outlines.request(Some(&second)).ok_or(CanvasError::Invalid("No retry"))?;
        outlines.complete(retry.run(context)?);
        assert!(outlines.get(Some(&second)).is_some() && outlines.get(Some(&first)).is_some());
        for left in 0..4 {
            let other = Path::rectangle([f64::from(left) + 20.0, 0.0, 40.0, 4.0])?
                .rasterize(extent, context)?;
            let work = outlines.request(Some(&other)).ok_or(CanvasError::Invalid("No work"))?;
            outlines.complete(work.run(context)?);
        }
        assert!(outlines.get(Some(&first)).is_none());
        Ok(())
    })
}

#[test]
fn ants_step_every_100_ms_and_hold_still_under_reduced_motion() {
    let at = |millis| MarchingAnts::at(Duration::from_millis(millis), false);
    assert_eq!([at(0).phase(), at(99).phase(), at(100).phase(), at(750).phase()], [0, 0, 1, 7]);
    assert_eq!(at(800).phase(), 0);
    assert_eq!(MarchingAnts::at(Duration::from_millis(750), true).phase(), 0);
    assert_eq!(
        MarchingAnts::next_step(Duration::from_millis(130), false),
        Some(Duration::from_millis(200))
    );
    assert_eq!(MarchingAnts::next_step(Duration::from_millis(130), true), None);
    let still = MarchingAnts::default();
    assert!(still.light(0.0) && still.light(3.9) && !still.light(4.0) && !still.light(7.9));
    assert!(!at(100).light(3.0) && at(100).light(7.5));
    assert!(outline_light(0.0) && outline_light(7.9) && !outline_light(8.0) && outline_light(12.0));
}

#[test]
fn presentation_overlays_own_tool_feedback_and_animate_only_with_ants() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(64, 64)?;
        let points = [[1.0, 1.0], [9.0, 1.0], [5.0, 8.0]];
        let drawn = PresentationOverlay::new(
            extent,
            None,
            Some(ToolOverlay::Outline(OverlayShape::Polygon(&points))),
            MarchingAnts::default(),
        );
        assert_eq!(drawn.outline, Some(OverlayOutline::Polygon(points.into())));
        assert!(!drawn.is_empty() && !drawn.animated());
        let square = Path::rectangle([2.0, 2.0, 9.0, 9.0])?.rasterize(extent, context)?;
        let contours = std::sync::Arc::new(CoverageContours::trace(&square, context)?);
        let moving = PresentationOverlay::new(
            extent,
            Some(contours),
            Some(ToolOverlay::SelectionOffset([3, -1])),
            MarchingAnts::default(),
        );
        assert_eq!((moving.selection_offset, moving.outline.is_none()), ([3, -1], true));
        assert!(moving.animated());
        assert!(PresentationOverlay::new(extent, None, None, MarchingAnts::default()).is_empty());
        Ok(())
    })
}
