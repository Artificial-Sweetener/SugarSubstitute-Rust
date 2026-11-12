// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canonical underlays repair only retained-primary coverage gaps.
use super::*;
use crate::{
    PreparedUpdate,
    tests::{harness, run_test, stamp},
};
use sugarsubstitute_canvas::{DamagePlan, Viewport};
use sugarsubstitute_contracts::execution::SurfaceId;
use sugarsubstitute_image_ops::Extent;

fn surface(
    extent: Extent,
    identity: FrameStamp,
    pixel: impl Fn(u32, u32) -> u32,
    context: &JobContext,
) -> Result<CpuSurface, RenderError> {
    CpuSurface::from_prepared(PreparedUpdate::prepare(
        identity,
        None,
        DamagePlan::full(extent),
        context.memory(),
        |region, pixels| {
            for y in 0..region.height() {
                for x in 0..region.width() {
                    pixels[(y * region.width() + x) as usize] = pixel(x, y);
                }
            }
            Ok(())
        },
    )?)
}

fn black() -> Result<Pixel, ImageError> {
    Pixel::linear(0.0, 0.0, 0.0, 1.0)
}

#[test]
fn primary_overlap_wins_and_underlay_repairs_only_missing_strips() -> harness::TestResult {
    run_test(|context| {
        let primary_extent = Extent::new(4, 2)?;
        let target_extent = Extent::new(6, 2)?;
        let primary = surface(primary_extent, stamp(3)?, |x, y| 0xff10_0000 | y << 8 | x, context)?;
        let underlay = surface(target_extent, stamp(2)?, |x, y| 0xff00_1000 | y << 8 | x, context)?;
        let target_view = Viewport::new(target_extent)?;
        let primary_map = ViewProjection::new(Viewport::new(primary_extent)?, target_view)?;
        let underlay_map = ViewProjection::new(Viewport::new(target_extent)?, target_view)?;
        let primary_samples = ProjectionSamples::new(primary_map, context)?;
        assert!(primary_samples.covers_pixel(3, 1));
        assert!(!primary_samples.covers_pixel(4, 1));
        assert!(!primary_samples.covers_pixel(6, 1));

        let projection = ProjectionBuffer::new(target_extent, context.memory(), context)?
            .project_over(
                stamp(4)?,
                &primary,
                primary_map,
                Some((&underlay, underlay_map)),
                black()?,
                context,
            )?;
        for y in 0..2 {
            for x in 0..6 {
                let expected =
                    if x < 4 { 0xff10_0000 | y << 8 | x } else { 0xff00_1000 | y << 8 | x };
                assert_eq!(projection.pixels()[(y * 6 + x) as usize], expected);
            }
        }
        assert_eq!(projection.source(), stamp(3)?);
        assert_eq!(projection.underlay(), Some(stamp(2)?));
        assert_eq!(primary.current(), Some(stamp(3)?));
        assert_eq!(underlay.current(), Some(stamp(2)?));
        Ok(())
    })
}

#[test]
fn zoomed_out_detail_uses_bilinear_underlay_for_its_lower_coverage() -> harness::TestResult {
    run_test(|context| {
        let primary_extent = Extent::new(4, 2)?;
        let underlay_extent = Extent::new(3, 1)?;
        let target_extent = Extent::new(6, 1)?;
        let primary = surface(primary_extent, stamp(3)?, |_, _| 0xffff_ffff, context)?;
        let colors = [0xff00_0000, 0xffff_0000, 0xff00_ff00];
        let underlay = surface(underlay_extent, stamp(2)?, |x, _| colors[x as usize], context)?;
        let target_view = Viewport::new(target_extent)?;
        let mut primary_view = Viewport::new(primary_extent)?;
        primary_view.zoom_at(2.0, [0.0; 2])?;
        let primary_map = ViewProjection::new(primary_view, target_view)?;
        let mut underlay_view = Viewport::new(underlay_extent)?;
        underlay_view.zoom_at(0.5, [0.0; 2])?;
        let underlay_map = ViewProjection::new(underlay_view, target_view)?;

        let projection = ProjectionBuffer::new(target_extent, context.memory(), context)?
            .project_over(
                stamp(4)?,
                &primary,
                primary_map,
                Some((&underlay, underlay_map)),
                black()?,
                context,
            )?;
        assert_eq!(projection.pixels()[0], 0xffff_ffff);
        assert_eq!(projection.pixels()[1], 0xffff_ffff);
        assert_eq!(projection.pixels()[2], 0xffbf_0000);
        let samples = ProjectionSamples::new(underlay_map, context)?;
        for x in 2..6 {
            assert_eq!(projection.pixels()[x as usize], samples.sample_pixel(&underlay, x, 0, 0));
        }
        Ok(())
    })
}

#[test]
fn magnified_underlay_is_nearest_and_foreign_or_stale_sources_are_rejected() -> harness::TestResult
{
    run_test(|context| {
        let primary_extent = Extent::new(1, 1)?;
        let underlay_extent = Extent::new(3, 1)?;
        let target_extent = Extent::new(9, 1)?;
        let primary = surface(primary_extent, stamp(3)?, |_, _| 0xffff_ffff, context)?;
        let colors = [0xff00_0000, 0xffff_0000, 0xff00_ff00];
        let underlay = surface(underlay_extent, stamp(2)?, |x, _| colors[x as usize], context)?;
        let mut target_view = Viewport::new(target_extent)?;
        target_view.zoom_at(3.0, [0.0; 2])?;
        let mut primary_view = Viewport::new(primary_extent)?;
        primary_view.zoom_at(3.0, [0.0; 2])?;
        let primary_map = ViewProjection::new(primary_view, target_view)?;
        let underlay_map = ViewProjection::new(Viewport::new(underlay_extent)?, target_view)?;
        let projection = ProjectionBuffer::new(target_extent, context.memory(), context)?
            .project_over(
                stamp(4)?,
                &primary,
                primary_map,
                Some((&underlay, underlay_map)),
                black()?,
                context,
            )?;
        assert_eq!(projection.pixels()[4], colors[1]);

        let foreign_stamp = FrameStamp {
            surface: SurfaceId::new(2).map_err(|_| RenderError::Invalid("Invalid test surface"))?,
            ..stamp(2)?
        };
        let foreign = surface(underlay_extent, foreign_stamp, |x, _| colors[x as usize], context)?;
        assert!(
            ProjectionBuffer::new(target_extent, context.memory(), context)?
                .project_over(
                    stamp(4)?,
                    &primary,
                    primary_map,
                    Some((&foreign, underlay_map)),
                    black()?,
                    context,
                )
                .is_err()
        );

        let stale = surface(underlay_extent, stamp(4)?, |x, _| colors[x as usize], context)?;
        assert!(matches!(
            ProjectionBuffer::new(target_extent, context.memory(), context)?.project_over(
                stamp(4)?,
                &primary,
                primary_map,
                Some((&stale, underlay_map)),
                black()?,
                context,
            ),
            Err(RenderError::StaleTarget)
        ));

        let mut shifted_target = target_view;
        shifted_target.pan_by([1.0, 0.0])?;
        let shifted = ViewProjection::new(Viewport::new(underlay_extent)?, shifted_target)?;
        assert!(
            ProjectionBuffer::new(target_extent, context.memory(), context)?
                .project_over(
                    stamp(4)?,
                    &primary,
                    primary_map,
                    Some((&underlay, shifted)),
                    black()?,
                    context,
                )
                .is_err()
        );

        let wrong_extent = ViewProjection::new(Viewport::new(Extent::new(2, 1)?)?, target_view)?;
        assert!(
            ProjectionBuffer::new(target_extent, context.memory(), context)?
                .project_over(
                    stamp(4)?,
                    &primary,
                    primary_map,
                    Some((&underlay, wrong_extent)),
                    black()?,
                    context,
                )
                .is_err()
        );
        Ok(())
    })
}

#[test]
fn repeated_primary_rows_keep_each_rows_own_underlay_repair() -> harness::TestResult {
    run_test(|context| {
        let primary_extent = Extent::new(2, 1)?;
        let target_extent = Extent::new(9, 3)?;
        let primary = surface(primary_extent, stamp(3)?, |x, _| 0xff10_0000 | x, context)?;
        let underlay = surface(target_extent, stamp(2)?, |x, y| 0xff00_1000 | y << 8 | x, context)?;
        let mut target_view = Viewport::new(target_extent)?;
        target_view.zoom_at(3.0, [0.0; 2])?;
        let primary_map = ViewProjection::new(Viewport::new(primary_extent)?, target_view)?;
        let underlay_map = ViewProjection::new(target_view, target_view)?;
        let samples = ProjectionSamples::new(primary_map, context)?;
        assert!(samples.rows().windows(2).all(|pair| pair[0] == pair[1]));

        let projection = ProjectionBuffer::new(target_extent, context.memory(), context)?
            .project_over(
                stamp(4)?,
                &primary,
                primary_map,
                Some((&underlay, underlay_map)),
                black()?,
                context,
            )?;
        for y in 0..3 {
            for x in 0..9 {
                let expected = if x < 6 { 0xff10_0000 | (x / 3) } else { 0xff00_1000 | y << 8 | x };
                assert_eq!(projection.pixels()[(y * 9 + x) as usize], expected, "pixel {x},{y}");
            }
        }
        Ok(())
    })
}
