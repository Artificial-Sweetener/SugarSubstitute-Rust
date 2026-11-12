// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retained CPU pixel repairs agree with independent full redraws in every pan direction.
use super::*;
use sugarsubstitute_canvas::{DamagePlan, FrameStamp, RenderRequest};
use sugarsubstitute_contracts::execution::{Generation, JobContext, SurfaceId};
use sugarsubstitute_image_ops::{Extent, ImageError, Region};
pub(crate) mod harness;
mod pipeline;

pub(crate) fn stamp(value: u64) -> Result<FrameStamp, RenderError> {
    let surface = SurfaceId::new(1).map_err(|_| RenderError::Invalid("Invalid test surface"))?;
    let generation =
        Generation::new(value).map_err(|_| RenderError::Invalid("Invalid test generation"))?;
    Ok(FrameStamp { surface, generation })
}
fn color(x: i64, y: i64) -> u32 {
    0xff000000
        | ((x.rem_euclid(256) as u32) << 16)
        | ((y.rem_euclid(256) as u32) << 8)
        | ((x + 3 * y).rem_euclid(256) as u32)
}
fn paint(region: Region, output: &mut [u32], dx: i32, dy: i32) {
    for y in region.y()..region.bottom() {
        for x in region.x()..region.right() {
            output
                [(y - region.y()) as usize * region.width() as usize + (x - region.x()) as usize] =
                color(i64::from(x) - i64::from(dx), i64::from(y) - i64::from(dy));
        }
    }
}
pub(crate) fn run_test(
    action: impl FnOnce(&JobContext) -> Result<(), RenderError> + Send + 'static,
) -> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        action(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

#[test]
fn overlapping_pan_blits_and_edge_repairs_match_full_redraws() -> harness::TestResult {
    run_test(|context| {
        let extent = Extent::new(11, 7)?;
        let first = stamp(1)?;
        let second = stamp(2)?;
        for dy in -9..=9 {
            for dx in -13..=13 {
                let mut surface = CpuSurface::new(first.surface, extent, context.memory())?;
                let initial = PreparedUpdate::prepare(
                    first,
                    None,
                    DamagePlan::full(extent),
                    context.memory(),
                    |region, output| {
                        paint(region, output, 0, 0);
                        Ok(())
                    },
                )?;
                surface.apply(&initial)?;
                let plan = DamagePlan::pan(extent, dx, dy)
                    .map_err(|_| RenderError::Invalid("Invalid test pan"))?;
                let repair = PreparedUpdate::prepare(
                    second,
                    Some(first),
                    plan,
                    context.memory(),
                    |region, output| {
                        paint(region, output, dx, dy);
                        Ok(())
                    },
                )?;
                let work = surface.apply(&repair)?;
                assert_eq!(work.repaired_pixels, plan.dirty().pixels());
                if dx != 0 || dy != 0 {
                    assert_eq!(work.repaired_pixels + work.reused_pixels, 77);
                }
                for y in 0..extent.height() {
                    for x in 0..extent.width() {
                        assert_eq!(
                            surface.pixels()[y as usize * 11 + x as usize],
                            color(i64::from(x) - i64::from(dx), i64::from(y) - i64::from(dy)),
                            "Pan {dx},{dy} at {x},{y}"
                        );
                    }
                }
            }
        }
        Ok(())
    })
}

#[test]
fn superseded_or_wrong_base_updates_do_not_modify_pixels() -> harness::TestResult {
    run_test(|context| {
        let extent = Extent::new(11, 7)?;
        let mut surface = CpuSurface::new(stamp(1)?.surface, extent, context.memory())?;
        let initial = PreparedUpdate::prepare(
            stamp(1)?,
            None,
            DamagePlan::full(extent),
            context.memory(),
            |_, output| {
                output.fill(0xff000011);
                Ok(())
            },
        )?;
        surface.apply(&initial)?;
        let second = PreparedUpdate::prepare(
            stamp(2)?,
            Some(stamp(1)?),
            DamagePlan::full(extent),
            context.memory(),
            |_, output| {
                output.fill(0xff000022);
                Ok(())
            },
        )?;
        let stale = PreparedUpdate::prepare(
            stamp(3)?,
            Some(stamp(1)?),
            DamagePlan::full(extent),
            context.memory(),
            |_, output| {
                output.fill(0xff000033);
                Ok(())
            },
        )?;
        surface.apply(&second)?;
        assert!(matches!(surface.apply(&stale), Err(RenderError::StaleBase)));
        assert!(matches!(surface.apply(&initial), Err(RenderError::StaleTarget)));
        assert!(surface.pixels().iter().all(|value| *value == 0xff000022));
        assert_eq!(surface.current(), Some(stamp(2)?));
        Ok(())
    })
}

#[test]
fn incomplete_initial_frames_and_invalid_pixels_never_publish() -> harness::TestResult {
    run_test(|context| {
        let extent = Extent::new(11, 7)?;
        let plan =
            DamagePlan::pan(extent, 2, 1).map_err(|_| RenderError::Invalid("Invalid test pan"))?;
        assert!(
            PreparedUpdate::prepare(stamp(1)?, None, plan, context.memory(), |_, _| Ok(()))
                .is_err()
        );
        assert!(
            PreparedUpdate::prepare(
                stamp(1)?,
                None,
                DamagePlan::full(extent),
                context.memory(),
                |_, output| {
                    // Color without coverage is not premultiplied presentation storage.
                    output.fill(0x00ff_0000);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(
            PreparedUpdate::prepare(
                stamp(1)?,
                Some(stamp(2)?),
                DamagePlan::full(extent),
                context.memory(),
                |_, _| Ok(())
            )
            .is_err()
        );
        Ok(())
    })
}

#[test]
fn rejected_surface_updates_preserve_retained_state() -> harness::TestResult {
    run_test(|context| {
        let extent = Extent::new(11, 7)?;
        let mut surface = CpuSurface::new(stamp(1)?.surface, extent, context.memory())?;
        let other = Extent::new(12, 7)?;
        let update = PreparedUpdate::prepare(
            stamp(1)?,
            None,
            DamagePlan::full(other),
            context.memory(),
            |_, output| {
                output.fill(0xff112233);
                Ok(())
            },
        )?;
        assert!(surface.apply(&update).is_err());
        assert_eq!(surface.current(), None);
        assert!(surface.pixels().iter().all(|value| *value == 0xff000000));
        Ok(())
    })
}
