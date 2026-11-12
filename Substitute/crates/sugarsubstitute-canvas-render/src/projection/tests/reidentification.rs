// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Late display admission preserves prepared pixels and their exclusive allocation.
use super::*;
use sugarsubstitute_contracts::execution::SurfaceId;

#[test]
fn reidentification_preserves_composed_pixels_cameras_sources_and_memory_claim()
-> harness::TestResult {
    run_test(|context| {
        let primary = surface(context)?;
        let underlay = CpuSurface::from_prepared(PreparedUpdate::prepare(
            stamp(2)?,
            None,
            DamagePlan::full(primary.extent()),
            context.memory(),
            |_, output| {
                output.fill(0xff12_3456);
                Ok(())
            },
        )?)?;
        let source_view = Viewport::new(primary.extent())?;
        let mut target_view = source_view;
        target_view.pan_by([3.0, 0.0])?;
        let view = ViewProjection::new(source_view, target_view)?;
        let baseline = available(context)?;
        let projection = ProjectionBuffer::new(primary.extent(), context.memory(), context)?
            .project_over(
                stamp(3)?,
                &primary,
                view,
                Some((&underlay, ViewProjection::new(target_view, target_view)?)),
                Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
                context,
            )?;
        let allocation = projection.pixels().as_ptr();
        let capacity = projection.buffer.pixels.capacity();
        let admitted = available(context)?;
        assert!(admitted < baseline);

        let projection = projection.reidentify(stamp(20)?)?;
        assert_eq!(projection.target(), stamp(20)?);
        assert_eq!(projection.source(), stamp(1)?);
        assert_eq!(projection.underlay(), Some(stamp(2)?));
        assert_eq!(projection.view(), view);
        assert_eq!(projection.pixels().as_ptr(), allocation);
        assert_eq!(projection.buffer.pixels.capacity(), capacity);
        assert_eq!(available(context)?, admitted);
        for y in 0..primary.extent().height() {
            for x in 0..primary.extent().width() {
                let expected = if x < 3 { 0xff12_3456 } else { color(x - 3, y) };
                assert_eq!(
                    projection.pixels()[(y * primary.extent().width() + x) as usize],
                    expected
                );
            }
        }
        assert_eq!(primary.current(), Some(stamp(1)?));
        assert_eq!(underlay.current(), Some(stamp(2)?));

        let buffer = projection.into_buffer();
        assert_eq!(buffer.extent(), primary.extent());
        assert_eq!(buffer.pixels.as_ptr(), allocation);
        assert_eq!(available(context)?, admitted);
        drop(buffer);
        assert_eq!(available(context)?, baseline);
        Ok(())
    })
}

#[test]
fn reidentification_rejects_equal_regressed_and_foreign_targets_and_releases_storage()
-> harness::TestResult {
    run_test(|context| {
        let base = surface(context)?;
        let camera = Viewport::new(base.extent())?;
        let view = ViewProjection::new(camera, camera)?;
        let foreign = FrameStamp {
            surface: SurfaceId::new(2).map_err(|_| RenderError::Invalid("Invalid test surface"))?,
            ..stamp(30)?
        };
        let baseline = available(context)?;
        for target in [stamp(20)?, stamp(19)?, foreign] {
            let projection = ProjectionBuffer::new(base.extent(), context.memory(), context)?
                .project(stamp(3)?, &base, view, Pixel::linear(0.0, 0.0, 0.0, 1.0)?, context)?
                .reidentify(stamp(20)?)?;
            assert_eq!(projection.underlay(), None);
            let result = projection.reidentify(target);
            if target.surface == stamp(20)?.surface {
                assert!(matches!(result, Err(RenderError::StaleTarget)));
            } else {
                assert!(matches!(result, Err(RenderError::Invalid(_))));
            }
            assert_eq!(available(context)?, baseline);
        }
        assert_eq!(base.current(), Some(stamp(1)?));
        Ok(())
    })
}

#[test]
fn in_place_reidentification_keeps_complete_custody_on_every_rejected_identity()
-> harness::TestResult {
    run_test(|context| {
        let base = surface(context)?;
        let camera = Viewport::new(base.extent())?;
        let view = ViewProjection::new(camera, camera)?;
        let baseline = available(context)?;
        let mut projection = ProjectionBuffer::new(base.extent(), context.memory(), context)?
            .project(stamp(3)?, &base, view, Pixel::linear(0.0, 0.0, 0.0, 1.0)?, context)?;
        let allocation = projection.pixels().as_ptr();
        let capacity = projection.buffer.pixels.capacity();
        let admitted = available(context)?;
        let foreign = FrameStamp {
            surface: SurfaceId::new(2).map_err(|_| RenderError::Invalid("Invalid test surface"))?,
            ..stamp(30)?
        };
        projection.reidentify_in_place(stamp(20)?)?;
        for target in [stamp(20)?, stamp(19)?, foreign] {
            let result = projection.reidentify_in_place(target);
            if target.surface == stamp(20)?.surface {
                assert!(matches!(result, Err(RenderError::StaleTarget)));
            } else {
                assert!(matches!(result, Err(RenderError::Invalid(_))));
            }
            assert_eq!(projection.target(), stamp(20)?);
            assert_eq!(projection.source(), stamp(1)?);
            assert_eq!(projection.underlay(), None);
            assert_eq!(projection.view(), view);
            assert_eq!(projection.pixels(), base.pixels());
            assert_eq!(projection.pixels().as_ptr(), allocation);
            assert_eq!(projection.buffer.pixels.capacity(), capacity);
            assert_eq!(available(context)?, admitted);
        }
        projection.reidentify_in_place(stamp(31)?)?;
        assert_eq!(projection.target(), stamp(31)?);
        assert_eq!(projection.pixels().as_ptr(), allocation);
        drop(projection.into_buffer());
        assert_eq!(available(context)?, baseline);
        Ok(())
    })
}
