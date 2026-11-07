// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify physical rounded patches against full-viewport operation graphs and retained sources.
use super::*;
use crate::{
    CpuProjection, CpuSurface, PreparedUpdate, ProjectionBuffer, RenderError,
    tests::{harness, stamp},
};
use sugarsubstitute_canvas::{DamagePlan, ViewProjection, Viewport};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{Extent, ImageError, Pixel};

fn run(
    action: impl FnOnce(&JobContext) -> Result<(), RenderError> + Send + 'static,
) -> harness::TestResult {
    harness::run(64 * 1024 * 1024, |context| {
        action(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}
fn pattern(x: u32, y: u32) -> u32 {
    u32::from_be_bytes([
        255,
        ((x * 73 + y * 29) % 256) as u8,
        ((x ^ y) % 256) as u8,
        if (x / 7 + y / 5).is_multiple_of(2) { 255 } else { 0 },
    ])
}
fn projection(
    extent: Extent,
    context: &JobContext,
) -> Result<(CpuSurface, CpuProjection), RenderError> {
    let mut source = CpuSurface::new(stamp(1)?.surface, extent, context.memory())?;
    let update = PreparedUpdate::prepare(
        stamp(1)?,
        None,
        DamagePlan::full(extent),
        context.memory(),
        |region, output| {
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    output[(y - region.y()) as usize * region.width() as usize
                        + (x - region.x()) as usize] = pattern(x, y);
                }
            }
            Ok(())
        },
    )?;
    source.apply(&update)?;
    let view = Viewport::new(extent)?;
    let frame = ProjectionBuffer::new(extent, context.memory(), context)?.project(
        stamp(2)?,
        &source,
        ViewProjection::new(view, view)?,
        Pixel::TRANSPARENT,
        context,
    )?;
    Ok((source, frame))
}

#[test]
fn backdrop_parameters_clip_original_masks_and_bound_adaptive_reduction() -> Result<(), RenderError>
{
    let viewport = Extent::new(333, 179)?;
    for (bounds, radius, sigma) in [
        ([0.0, 0.0, 0.0, 10.0], 0.0, 1.0),
        ([f64::NAN, 0.0, 10.0, 10.0], 0.0, 1.0),
        ([0.0, 0.0, 10.0, 10.0], -1.0, 1.0),
        ([0.0, 0.0, 10.0, 10.0], 0.0, f32::INFINITY),
    ] {
        assert!(BackdropRegion::new(bounds, radius, sigma).is_err());
    }
    let bar = BackdropRegion::new([-5.25, 3.5, 100.0, 30.0], 8.0, 150.0)?;
    let plan = BackdropPlan::new(viewport, &[bar])?;
    let crop = plan.regions()[0];
    assert_eq!(crop.bounds(), [-5.25, 3.5, 100.0, 30.0]);
    assert_eq!(crop.crop().x(), 0);
    assert_eq!(crop.reduction(), 8);
    assert_eq!(crop.blur().sigma(), 18.75);
    assert_eq!(crop.coverage(0, 15), 1.0);
    assert_eq!(crop.coverage(94, 3), 0.0);
    assert!(BackdropPlan::new(viewport, &[bar; 3]).is_err());
    let zero = BackdropRegion::new([0.0, 0.0, 10.0, 10.0], 0.0, 0.0)?;
    assert!(BackdropPlan::new(viewport, &[zero])?.regions().is_empty());
    assert_eq!(BackdropPlan::new(viewport, &[])?.cpu_scratch_bytes(), 0);
    Ok(())
}

#[test]
fn backdrop_retention_hint_matches_actual_local_dag_and_explicit_cpu_funding() -> harness::TestResult
{
    run(|context| {
        use sugarsubstitute_image_ops::{AssetId, Operation, TILE_EDGE};
        for (viewport, bounds, sigma) in [
            (Extent::new(1920, 1080)?, [660.0, 18.0, 600.0, 40.0], 10.0),
            (Extent::new(333, 211)?, [-3.5, 19.25, 249.0, 49.0], 150.0),
        ] {
            let plan = BackdropPlan::new(viewport, &[BackdropRegion::new(bounds, 12.0, sigma)?])?;
            let crop = plan.regions()[0];
            let prepared = crop.graph(AssetId([7; 32]), context.memory())?;
            let mut node = prepared.output;
            let mut actual = 0_u64;
            loop {
                actual += prepared.graph.extent(node)?.region().tile_count();
                match prepared.graph.operation(node)? {
                    Operation::Raster { .. } => break,
                    Operation::Resample { input, .. } | Operation::GaussianBlur { input, .. } => {
                        node = input
                    }
                    _ => {
                        return Err(RenderError::Invalid(
                            "Unexpected backdrop operation inventory",
                        ));
                    }
                }
            }
            assert_eq!(crop.retained_tiles(), actual.min(4096) as usize);
            let cache = crop.retained_tiles() as u64
                * (u64::from(TILE_EDGE).pow(2) * size_of::<Pixel>() as u64 + 256);
            assert!(plan.cpu_scratch_bytes() >= crop.crop().pixels() * 4 + cache + 8 * 1024 * 1024);
            if sigma == 10.0 {
                assert_eq!(crop.retained_tiles(), 19);
            }
        }
        Ok(())
    })
}

/// A rounded patch changes actual display pixels while source/frame/camera identities stay exact.
#[test]
fn backdrop_patches_preserve_canonical_pixels_and_every_pixel_outside_original_mask()
-> harness::TestResult {
    run(|context| {
        let extent = Extent::new(181, 107)?;
        let (source, mut frame) = projection(extent, context)?;
        let original = frame.pixels().to_vec();
        let identity = (frame.target(), frame.source(), frame.underlay(), frame.view());
        let plan = BackdropPlan::new(
            extent,
            &[BackdropRegion::new([21.25, 19.75, 119.5, 39.5], 12.0, 6.0)?],
        )?;
        frame.paint_backdrops(&plan, context)?;
        assert_eq!((frame.target(), frame.source(), frame.underlay(), frame.view()), identity);
        assert_eq!(source.pixels(), original);
        let crop = plan.regions()[0];
        let mut changed = 0;
        for y in 0..extent.height() {
            for x in 0..extent.width() {
                let index = y as usize * extent.width() as usize + x as usize;
                if crop.coverage(x, y) == 0.0 {
                    assert_eq!(frame.pixels()[index], original[index]);
                } else if frame.pixels()[index] != original[index] {
                    changed += 1;
                }
            }
        }
        assert!(changed > 100, "The actual acrylic interior remained sharp");
        Ok(())
    })
}

/// Odd extents, fractional bars and physical edges keep the full-viewport global sampling phase.
#[test]
fn backdrop_crops_match_full_viewport_reference_without_artificial_edge_influence()
-> harness::TestResult {
    run(|context| {
        let extent = Extent::new(521, 317)?;
        for bounds in
            [[223.25, 130.75, 73.5, 31.25], [-5.25, 1.5, 84.0, 33.5], [457.5, 280.25, 90.0, 49.0]]
        {
            let (_, mut actual) = projection(extent, context)?;
            let (_, mut reference) = projection(extent, context)?;
            let original = actual.pixels().to_vec();
            let plan = BackdropPlan::new(extent, &[BackdropRegion::new(bounds, 8.5, 3.25)?])?;
            let full = BackdropPlan::new(
                extent,
                &[BackdropRegion::new([0.0, 0.0, 521.0, 317.0], 0.0, 3.25)?],
            )?;
            reference.paint_backdrops(&full, context)?;
            actual.paint_backdrops(&plan, context)?;
            let mask = plan.regions()[0];
            let mut compared = 0;
            for y in 0..extent.height() {
                for x in 0..extent.width() {
                    let index = y as usize * extent.width() as usize + x as usize;
                    let coverage = mask.coverage(x, y);
                    let expected =
                        super::cpu::masked(original[index], reference.pixels()[index], coverage);
                    for (actual, expected) in
                        actual.pixels()[index].to_be_bytes().into_iter().zip(expected.to_be_bytes())
                    {
                        assert!(
                            actual.abs_diff(expected) <= 1,
                            "Cropped acrylic differs at {x},{y}, {bounds:?}"
                        );
                    }
                    compared += usize::from(coverage != 0.0);
                }
            }
            assert!(compared > 100);
        }
        Ok(())
    })
}

#[test]
fn backdrop_overlaps_read_the_clean_projection_instead_of_prior_blurred_patches()
-> harness::TestResult {
    run(|context| {
        let extent = Extent::new(221, 137)?;
        let first = BackdropRegion::new([21.0, 29.0, 140.0, 44.0], 5.0, 3.0)?;
        let second = BackdropRegion::new([91.0, 42.0, 110.0, 53.0], 0.0, 9.0)?;
        let (_, mut combined) = projection(extent, context)?;
        let (_, mut independent) = projection(extent, context)?;
        combined.paint_backdrops(&BackdropPlan::new(extent, &[first, second])?, context)?;
        let plan = BackdropPlan::new(extent, &[second])?;
        independent.paint_backdrops(&plan, context)?;
        let mask = plan.regions()[0];
        for y in 42..95 {
            for x in 91..201 {
                assert_eq!(mask.coverage(x, y), 1.0);
                let index = y as usize * extent.width() as usize + x as usize;
                assert_eq!(combined.pixels()[index], independent.pixels()[index]);
            }
        }
        Ok(())
    })
}

#[test]
fn backdrop_foreign_extent_expired_control_and_no_memory_reject_before_patching()
-> harness::TestResult {
    run(|context| {
        let extent = Extent::new(131, 79)?;
        let (_, mut frame) = projection(extent, context)?;
        let original = frame.pixels().to_vec();
        let bar = BackdropRegion::new([11.0, 9.0, 90.0, 30.0], 6.0, 5.0)?;
        assert!(
            frame
                .paint_backdrops(&BackdropPlan::new(Extent::new(132, 79)?, &[bar])?, context)
                .is_err()
        );
        let plan = BackdropPlan::new(extent, &[bar])?;
        assert!(matches!(
            frame.paint_backdrops(&plan, &context.bounded_until(context.now())),
            Err(RenderError::Image(ImageError::Deadline))
        ));
        let available = context
            .memory()
            .claim(u64::MAX)
            .err()
            .ok_or(RenderError::Invalid("Unbounded test claim accepted"))?
            .available_bytes;
        let held = context.memory().claim(available - 16).map_err(ImageError::from)?;
        assert!(matches!(
            frame.paint_backdrops(&plan, context),
            Err(RenderError::Image(ImageError::Memory(_)))
        ));
        assert_eq!(frame.pixels(), original);
        drop(held);
        frame.paint_backdrops(&plan, context)?;
        assert_ne!(frame.pixels(), original);
        Ok(())
    })
}

#[test]
fn backdrop_scope_cancellation_rejects_before_mutating_unpublished_pixels() -> harness::TestResult {
    let outcome = harness::run_with_scope(64 * 1024 * 1024, |context, scope| {
        let result = (|| -> Result<(), RenderError> {
            let extent = Extent::new(131, 79)?;
            let (_, mut frame) = projection(extent, context)?;
            let original = frame.pixels().to_vec();
            let plan = BackdropPlan::new(
                extent,
                &[BackdropRegion::new([11.0, 9.0, 90.0, 30.0], 6.0, 5.0)?],
            )?;
            scope.cancel();
            assert!(matches!(
                frame.paint_backdrops(&plan, context),
                Err(RenderError::Image(ImageError::Cancelled))
            ));
            assert_eq!(frame.pixels(), original);
            Ok(())
        })();
        result.map_err(|cause| ImageError::Source(Box::new(cause)))
    })?;
    assert!(outcome.is_none(), "The real executor did not acknowledge scope cancellation");
    Ok(())
}

#[test]
fn backdrop_reported_scratch_funds_both_crops_and_adaptive_dag_with_exact_admission()
-> harness::TestResult {
    run(|context| {
        let extent = Extent::new(333, 211)?;
        let (source, mut frame) = projection(extent, context)?;
        let plan = BackdropPlan::new(
            extent,
            &[
                BackdropRegion::new([21.25, 17.5, 241.0, 41.5], 12.0, 12.0)?,
                BackdropRegion::new([73.0, 142.0, 249.0, 49.0], 9.0, 150.0)?,
            ],
        )?;
        let available = context
            .memory()
            .claim(u64::MAX)
            .err()
            .ok_or(RenderError::Invalid("Unbounded test claim accepted"))?
            .available_bytes;
        assert!(available > plan.cpu_scratch_bytes());
        let held = context
            .memory()
            .claim(available - plan.cpu_scratch_bytes())
            .map_err(ImageError::from)?;
        frame.paint_backdrops(&plan, context)?;
        assert!((20..60).any(|y| (30..250).any(|x| {
            let index = y * extent.width() as usize + x;
            frame.pixels()[index] != source.pixels()[index]
        })));
        drop(held);
        Ok(())
    })
}
