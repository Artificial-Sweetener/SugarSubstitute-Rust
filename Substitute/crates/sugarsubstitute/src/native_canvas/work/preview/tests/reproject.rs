// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify candidate resize preserves backing identity, Fit and complete prior pixels on failure.

use super::*;

#[test]
fn candidate_reprojection_keeps_canonical_backing_and_commits_only_complete_fit_pixels()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let mut fixture = None;
    harness::run(directory, RETAINED_BYTES, false, false, move |context, _, retained| {
        if fixture.is_none() {
            let resources = retained.clone();
            let scene =
                scene(7, Extent::new(32, 16)?, Pixel::linear(0.0, 1.0, 0.0, 1.0)?, context)?;
            fixture = Some(PreparePreview::new(
                scene,
                &resources,
                stamp(9, 10)?,
                stamp(9, 11)?,
                Extent::new(96, 96)?,
                DisplayBackdrops::empty(Extent::new(96, 96)?)?,
            )?);
        }
        let CpuStep::Complete(mut ready) = fixture
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Resize fixture missing"))?
            .step(context)?
        else {
            return Ok(CpuStep::Yield);
        };
        let backing = ready.backing.clone();
        let original_source = ready.drawing.source;
        let original_target = ready.drawing.target;
        let original_view = ready.drawing.view;
        let computed = ready
            .resources
            .engine
            .lock()
            .map_err(|_| WorkError::invalid("Preview engine poisoned"))?
            .renderer
            .as_ref()
            .and_then(|renderer| match renderer {
                Renderer::Cpu(renderer) => Some(renderer.stats().computed_pixels),
                Renderer::Gpu(_) => None,
            })
            .ok_or_else(|| WorkError::invalid("Preview renderer missing"))?;
        let error = ready
            .reproject(
                stamp(9, 12)?,
                Extent::new(128, 72)?,
                DisplayBackdrops::empty(Extent::new(128, 72)?)?,
                &context.bounded_until(context.now()),
            )
            .err()
            .ok_or_else(|| WorkError::invalid("Expired resize succeeded"))?;
        assert!(matches!(
            error.source().and_then(|cause| cause.downcast_ref::<ImageError>()),
            Some(ImageError::Deadline)
        ));
        assert_eq!(ready.drawing.target, original_target);
        assert_eq!(ready.drawing.view, original_view);
        let remaining = available(&ready.resources.memory)?;
        let pressure = ready.resources.memory.claim(remaining - 1024)?;
        let error = ready
            .reproject(
                stamp(9, 12)?,
                Extent::new(16384, 16384)?,
                DisplayBackdrops::empty(Extent::new(16384, 16384)?)?,
                context,
            )
            .err()
            .ok_or_else(|| WorkError::invalid("Over-budget resize succeeded"))?;
        assert!(contains_memory_error(&error));
        assert_eq!(ready.drawing.target, original_target);
        assert_eq!(ready.drawing.view, original_view);
        drop(pressure);
        ready.reproject(
            stamp(9, 12)?,
            Extent::new(128, 72)?,
            DisplayBackdrops::empty(Extent::new(128, 72)?)?,
            context,
        )?;
        assert!(Arc::ptr_eq(&ready.backing, &backing));
        assert_eq!(ready.backing.current(), Some(original_source));
        assert_eq!(ready.drawing.source, original_source);
        assert_eq!(ready.drawing.target, stamp(9, 12)?);
        assert_eq!(ready.drawing.view.surface(), Extent::new(128, 72)?);
        assert_eq!(ready.drawing.view.device_at([16.0, 8.0])?, [64.0, 36.0]);
        assert_eq!(ready.spare_buffer.extent(), Extent::new(128, 72)?);
        let DrawPixels::Cpu(projection) = &ready.drawing.pixels else {
            return Err(WorkError::invalid("Resized candidate returned GPU pixels"));
        };
        assert_eq!(projection.pixels().len(), 128 * 72);
        assert_eq!(projection.pixels()[36 * 128 + 64], 0xff00ff00);
        assert!(
            ready
                .reproject(
                    stamp(9, 12)?,
                    Extent::new(128, 72)?,
                    DisplayBackdrops::empty(Extent::new(128, 72)?)?,
                    context
                )
                .is_err()
        );
        assert!(
            ready
                .reproject(
                    stamp(8, 13)?,
                    Extent::new(128, 72)?,
                    DisplayBackdrops::empty(Extent::new(128, 72)?)?,
                    context
                )
                .is_err()
        );
        let renderer_computed = ready
            .resources
            .engine
            .lock()
            .map_err(|_| WorkError::invalid("Preview engine poisoned"))?
            .renderer
            .as_ref()
            .and_then(|renderer| match renderer {
                Renderer::Cpu(renderer) => Some(renderer.stats().computed_pixels),
                Renderer::Gpu(_) => None,
            })
            .ok_or_else(|| WorkError::invalid("Preview renderer missing"))?;
        assert_eq!(renderer_computed, computed);
        drop(ready);
        drop(fixture.take());
        Ok(CpuStep::Complete(()))
    })
}
