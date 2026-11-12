// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pixel-edge boundary fixtures, boundary caching and ant animation.
use super::*;
use crate::{CanvasError, tests::harness};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{
    CombineMode, Coverage, FULL_COVERAGE, ImageError, Path, Region, coverage_from_u8,
};

fn trace(coverage: &Coverage, context: &JobContext) -> Result<CoverageBoundary, CanvasError> {
    CoverageBoundary::trace(coverage, context)
}

fn run(
    test: impl FnOnce(&JobContext) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(16 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

fn horizontal(x: u32, y: u32, length: u32) -> BoundaryRun {
    BoundaryRun { start: [x, y], length, vertical: false }
}

fn vertical(x: u32, y: u32, length: u32) -> BoundaryRun {
    BoundaryRun { start: [x, y], length, vertical: true }
}

/// CuteCanvas's `SelectionBoundaryBuilder` over the whole extent, transcribed literally.
fn reference(coverage: &Coverage) -> Vec<BoundaryRun> {
    let extent = coverage.extent();
    let (width, height) = (extent.width() as usize, extent.height() as usize);
    let selected = |x: usize, y: usize| {
        sugarsubstitute_image_ops::coverage_to_u8(coverage.value(x as u32, y as u32)) >= 128
    };
    let runs = |values: Vec<bool>| {
        let mut found = Vec::new();
        let mut start = None;
        for (index, value) in values.iter().chain([&false]).enumerate() {
            match (*value, start) {
                (true, None) => start = Some(index),
                (false, Some(first)) => {
                    found.push((first as u32, index as u32));
                    start = None;
                }
                _ => {}
            }
        }
        found
    };
    let mut result = Vec::new();
    for y in 0..=height {
        let edges = (0..width)
            .map(|x| (y > 0 && selected(x, y - 1)) != (y < height && selected(x, y)))
            .collect();
        for (start, end) in runs(edges) {
            result.push(horizontal(start, y as u32, end - start));
        }
    }
    for x in 0..=width {
        let edges = (0..height)
            .map(|y| (x > 0 && selected(x - 1, y)) != (x < width && selected(x, y)))
            .collect();
        for (start, end) in runs(edges) {
            result.push(vertical(x as u32, start, end - start));
        }
    }
    result
}

#[test]
fn boundaries_are_pixel_edge_runs_in_cutecanvas_order() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(300, 200)?;
        let square = Path::rectangle([10.0, 20.0, 50.0, 70.0])?.rasterize(extent, context)?;
        let boundary = trace(&square, context)?;
        assert_eq!(
            boundary.runs(),
            &[
                horizontal(10, 20, 40),
                horizontal(10, 70, 40),
                vertical(10, 20, 50),
                vertical(50, 20, 50),
            ]
        );
        assert_eq!(boundary.fingerprint(), square.fingerprint());
        assert_eq!(boundary.runs()[3].end(), [50, 70]);
        // Selected pixels on the extent's edge are bounded by the edge itself.
        let everything = Coverage::new(extent, FULL_COVERAGE, context.memory())?;
        assert_eq!(
            trace(&everything, context)?.runs(),
            &[
                horizontal(0, 0, 300),
                horizontal(0, 200, 300),
                vertical(0, 0, 200),
                vertical(300, 0, 200),
            ]
        );
        assert!(trace(&Coverage::new(extent, 0, context.memory())?, context)?.is_empty());
        Ok(())
    })
}

#[test]
fn holes_islands_and_anti_aliased_edges_match_the_reference_builder() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(200, 150)?;
        let outer = Path::rectangle([20.0, 20.0, 120.0, 120.0])?.rasterize(extent, context)?;
        let hole = Path::ellipse([40.5, 45.25, 90.0, 80.0])?.rasterize(extent, context)?;
        let island = Path::polygon(vec![[130.0, 10.0], [195.3, 60.7], [140.2, 140.0]])?
            .rasterize(extent, context)?;
        let shapes = outer.combine(&hole, CombineMode::Subtract, context)?.combine(
            &island,
            CombineMode::Add,
            context,
        )?;
        let boundary = trace(&shapes, context)?;
        assert_eq!(boundary.runs(), reference(&shapes).as_slice());
        // The hole adds its own runs inside the square.
        assert!(
            boundary
                .runs()
                .iter()
                .any(|run| run.start[1] > 45 && run.start[1] < 80 && !run.vertical)
        );
        Ok(())
    })
}

#[test]
fn partial_coverage_is_selected_from_eight_bit_128() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(16, 4)?;
        let mut coverage = Coverage::new(extent, 0, context.memory())?;
        let values = [127_u8, 128, 200, 128, 127, 255, 0, 128].map(coverage_from_u8);
        coverage.write(Region::new(2, 1, 8, 1)?, &values, context.memory())?;
        let boundary = trace(&coverage, context)?;
        assert_eq!(boundary.runs(), reference(&coverage).as_slice());
        assert_eq!(
            boundary.runs(),
            &[
                horizontal(3, 1, 3),
                horizontal(7, 1, 1),
                horizontal(9, 1, 1),
                horizontal(3, 2, 3),
                horizontal(7, 2, 1),
                horizontal(9, 2, 1),
                vertical(3, 1, 1),
                vertical(6, 1, 1),
                vertical(7, 1, 1),
                vertical(8, 1, 1),
                vertical(9, 1, 1),
                vertical(10, 1, 1),
            ]
        );
        // Diagonal neighbours stay separate, with no saddle joining them.
        let mut diagonal = Coverage::new(extent, 0, context.memory())?;
        diagonal.write(
            Region::new(2, 1, 2, 2)?,
            &[FULL_COVERAGE, 0, 0, FULL_COVERAGE],
            context.memory(),
        )?;
        assert_eq!(trace(&diagonal, context)?.runs(), reference(&diagonal).as_slice());
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
            &[],
            MarchingAnts::default(),
        );
        assert_eq!(drawn.outline, Some(OverlayOutline::Polygon(points.into())));
        assert!(!drawn.is_empty() && !drawn.animated());
        let square = Path::rectangle([2.0, 2.0, 9.0, 9.0])?.rasterize(extent, context)?;
        let boundary = std::sync::Arc::new(CoverageBoundary::trace(&square, context)?);
        let moving = PresentationOverlay::new(
            extent,
            Some(boundary),
            Some(ToolOverlay::SelectionOffset([3, -1])),
            &[],
            MarchingAnts::default(),
        );
        assert_eq!((moving.selection_offset, moving.outline.is_none()), ([3, -1], true));
        assert!(moving.animated());
        assert!(
            PresentationOverlay::new(extent, None, None, &[], MarchingAnts::default()).is_empty()
        );
        let corners = [[1.0, 1.0], [9.0, 1.0], [9.0, 8.0], [1.0, 8.0]];
        let framed = PresentationOverlay::new(
            extent,
            None,
            Some(ToolOverlay::Frame(corners)),
            &[],
            MarchingAnts::default(),
        );
        assert_eq!(framed.guides, Some(OverlayGuides::Frame(corners)));
        assert!(!framed.is_empty() && !framed.animated());
        let seams = [OverlaySeam {
            start: [5.0, 0.0],
            end: [5.0, 9.0],
            focused: true,
            handle: Some(SeamHandle::Middle),
            enabled: [true; 3],
        }];
        let seamed = PresentationOverlay::new(
            extent,
            None,
            Some(ToolOverlay::Seams(&seams)),
            &[],
            MarchingAnts::default(),
        );
        assert_eq!(seamed.guides, Some(OverlayGuides::Seams(seams.into())));
        let guide = SnapGuide { start: [0.0, 4.0], end: [9.0, 4.0] };
        let snapped =
            PresentationOverlay::new(extent, None, None, &[guide], MarchingAnts::default());
        assert_eq!(&*snapped.snaps, [guide]);
        assert!(!snapped.is_empty() && !snapped.animated());
        Ok(())
    })
}
