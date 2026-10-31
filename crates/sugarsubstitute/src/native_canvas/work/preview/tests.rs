// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify isolated candidate preparation, centered Fit evidence and worker-owned failure custody.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_image_ops::VectorRectangle;

mod harness;
mod reproject;

fn stamp(surface: u64, generation: u64) -> Result<FrameStamp, WorkError> {
    Ok(FrameStamp { surface: SurfaceId::new(surface)?, generation: Generation::new(generation)? })
}

fn scene(
    identity: u64,
    extent: Extent,
    color: Pixel,
    context: &JobContext,
) -> Result<Scene, WorkError> {
    let mut document = Document::new(identity, extent, context.memory())?;
    let rectangle = VectorRectangle::new(
        [0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())],
        color,
    )?;
    document.apply(DocumentEdit::Insert {
        index: 0,
        layer: Layer::new(LayerId::new(1)?, LayerContent::VectorRectangle(rectangle))?,
    })?;
    Ok(Scene { document, source: None, source_revision: identity, original: None, compared: None })
}

fn available(memory: &MemoryLease) -> Result<u64, WorkError> {
    memory
        .claim(u64::MAX)
        .err()
        .map(|cause| cause.available_bytes)
        .ok_or_else(|| WorkError::invalid("Unbounded memory claim succeeded"))
}

fn contains_memory_error(error: &(dyn Error + 'static)) -> bool {
    error.downcast_ref::<MemoryClaimError>().is_some()
        || error.source().is_some_and(contains_memory_error)
}

#[test]
fn interleaved_candidate_and_active_final_keep_separate_renderers_and_pixels()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let mut fixture = None;
    let mut active_result = None;
    let mut preview_result = None;
    harness::run(directory, RETAINED_BYTES, false, false, move |context, _, retained| {
        if fixture.is_none() {
            let resources = retained.clone();
            let active_scene =
                scene(11, Extent::new(32, 16)?, Pixel::linear(1.0, 0.0, 0.0, 1.0)?, context)?;
            let candidate_scene =
                scene(22, Extent::new(16, 32)?, Pixel::linear(0.0, 0.0, 1.0, 1.0)?, context)?;
            let mut preview = PreparePreview::new(
                active_scene,
                &resources,
                stamp(1, 1)?,
                stamp(1, 2)?,
                Extent::new(96, 64)?,
            )?;
            let mut active = preview.prepare.take().ok_or_else(|| {
                WorkError::invalid("Fixture active preparation was not constructed")
            })?;
            active.resources = resources.clone();
            active.replace_buffers = false;
            let preview = PreparePreview::new(
                candidate_scene,
                &resources,
                stamp(2, 3)?,
                stamp(2, 4)?,
                Extent::new(96, 64)?,
            )?;
            assert!(!Arc::ptr_eq(&resources.engine, &preview.resources.engine));
            fixture = Some((resources, active, preview));
        }
        let (resources, active, preview) = fixture
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Interleaved fixture is unavailable"))?;
        if active_result.is_none()
            && let CpuStep::Complete(packet) = active.step(context)?
        {
            active_result = Some(packet);
        }
        if preview_result.is_none()
            && let CpuStep::Complete(ready) = preview.step(context)?
        {
            preview_result = Some(ready);
        }
        assert_eq!(
            resources
                .engine
                .lock()
                .map_err(|_| WorkError::invalid("Active engine poisoned"))?
                .source_revision,
            Some(11)
        );
        assert_eq!(
            preview
                .resources
                .engine
                .lock()
                .map_err(|_| WorkError::invalid("Candidate engine poisoned"))?
                .source_revision,
            Some(22)
        );
        let (Some(packet), Some(ready)) = (&active_result, &preview_result) else {
            return Ok(CpuStep::Yield);
        };
        let Pixels::Cpu(CpuPacket::Replace { surface }) = &packet.pixels else {
            return Err(WorkError::invalid("Active preparation did not replace CPU pixels"));
        };
        assert_eq!(surface.current(), Some(stamp(1, 1)?));
        let center = surface.extent().height() as usize / 2 * surface.extent().width() as usize
            + surface.extent().width() as usize / 2;
        assert_eq!(surface.pixels()[center], 0xffff0000);
        assert_eq!(ready.backing.current(), Some(stamp(2, 3)?));
        assert_eq!(ready.drawing.source_revision, 22);
        let DrawPixels::Cpu(projection) = &ready.drawing.pixels else {
            return Err(WorkError::invalid("Candidate returned GPU pixels"));
        };
        assert_eq!(projection.pixels()[32 * 96 + 48], 0xff0000ff);
        assert_eq!(projection.source(), stamp(2, 3)?);
        drop(preview_result.take());
        drop(active_result.take());
        drop(fixture.take());
        Ok(CpuStep::Complete(()))
    })
}

#[test]
fn large_portrait_candidate_uses_existing_backing_cap_and_physical_fit_camera()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    harness::run(directory, RETAINED_BYTES, false, false, move |context, _, retained| {
        let resources = retained.clone();
        let scene =
            scene(3, Extent::new(1920, 3024)?, Pixel::linear(1.0, 1.0, 1.0, 1.0)?, context)?;
        let preview = PreparePreview::new(
            scene,
            &resources,
            stamp(2, 1)?,
            stamp(2, 2)?,
            Extent::new(3840, 2160)?,
        )?;
        assert_eq!(preview.coverage.viewport().surface(), Extent::new(651, 1024)?);
        assert_eq!(preview.view.surface(), Extent::new(3840, 2160)?);
        assert_eq!(preview.view.device_at([960.0, 1512.0])?, [1920.0, 1080.0]);
        let prepare = preview
            .prepare
            .as_ref()
            .ok_or_else(|| WorkError::invalid("Portrait preparation missing"))?;
        assert_eq!(prepare.reconstruction, Reconstruction::Finished(Filter::Lanczos3));
        assert!(prepare.reconstruction.uses_canonical_pyramid());
        assert_eq!(prepare.view, preview.coverage.viewport());
        drop(preview);
        Ok(CpuStep::Complete(()))
    })
}

#[test]
fn candidate_retains_centered_fit_pixels_and_complete_identity_evidence()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let mut fixture = None;
    let mut yields = 0;
    harness::run(directory, RETAINED_BYTES, true, false, move |context, _, retained| {
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
            )?);
        }
        match fixture
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Fit fixture missing"))?
            .step(context)?
        {
            CpuStep::Yield => {
                yields += 1;
                Ok(CpuStep::Yield)
            }
            CpuStep::Complete(ready) => {
                assert!(yields >= 3);
                let mut expected = Viewport::new(Extent::new(96, 96)?)?;
                expected.fit(Extent::new(32, 16)?)?;
                assert_eq!(ready.drawing.view, expected);
                assert_eq!(ready.drawing.view.device_at([16.0, 8.0])?, [48.0, 48.0]);
                assert_eq!(ready.drawing.target, stamp(9, 11)?);
                assert_eq!(ready.drawing.source, stamp(9, 10)?);
                assert_eq!(ready.drawing.document, (7, 2));
                assert_eq!(ready.scene.source_revision, 7);
                assert_eq!(ready.drawing.tier, BackingTier::Fallback);
                assert_eq!(ready.reconstruction, Reconstruction::Finished(Filter::Lanczos3));
                assert!(ready.drawing.underlay.is_none());
                assert!(ready.drawing.verified);
                assert_eq!(ready.spare_buffer.extent(), Extent::new(96, 96)?);
                assert!(ready.resources.capture);
                assert!(
                    ready
                        .resources
                        .engine
                        .lock()
                        .map_err(|_| WorkError::invalid("Candidate engine poisoned"))?
                        .device
                        .is_none()
                );
                assert!(ready.sampling_scale > 0.0);
                assert!(ready.stats.computed_pixels > 0);
                assert!(ready.elapsed >= std::time::Duration::ZERO);
                assert!(ready.source_io.is_none());
                let DrawPixels::Cpu(projection) = &ready.drawing.pixels else {
                    return Err(WorkError::invalid("Fit projection returned GPU pixels"));
                };
                assert_eq!(projection.pixels()[48 * 96 + 48], 0xff00ff00);
                assert_ne!(projection.pixels()[48], 0xff00ff00);
                assert_eq!(ready.drawing.coverage, coverage::fallback(Extent::new(32, 16)?)?);
                drop(ready);
                drop(fixture.take());
                Ok(CpuStep::Complete(()))
            }
        }
    })
}

#[test]
fn candidate_cancel_and_deadline_stop_before_more_allocation_and_retire_on_worker()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    harness::run(directory, RETAINED_BYTES, false, true, move |context, scope, retained| {
        let resources = retained.clone();
        let baseline = available(&resources.memory)?;
        let scene = scene(3, Extent::new(32, 16)?, Pixel::linear(1.0, 1.0, 1.0, 1.0)?, context)?;
        let mut preview = PreparePreview::new(
            scene,
            &resources,
            stamp(2, 1)?,
            stamp(2, 2)?,
            Extent::new(96, 96)?,
        )?;
        assert!(matches!(preview.step(context)?, CpuStep::Yield));
        let admitted = available(&resources.memory)?;
        let error = preview
            .step(&context.bounded_until(context.now()))
            .err()
            .ok_or_else(|| WorkError::invalid("Expired candidate advanced"))?;
        assert!(matches!(
            error.source().and_then(|cause| cause.downcast_ref::<ImageError>()),
            Some(ImageError::Deadline)
        ));
        assert_eq!(available(&resources.memory)?, admitted);
        scope.cancel();
        let error = preview
            .step(context)
            .err()
            .ok_or_else(|| WorkError::invalid("Cancelled candidate advanced"))?;
        assert!(matches!(
            error.source().and_then(|cause| cause.downcast_ref::<ImageError>()),
            Some(ImageError::Cancelled)
        ));
        assert_eq!(available(&resources.memory)?, admitted);
        drop(preview);
        assert_eq!(available(&resources.memory)?, baseline);
        Ok(CpuStep::Complete(()))
    })
}

#[test]
fn candidate_budget_failure_preserves_admission_cause_and_active_engine()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    harness::run(directory, RETAINED_BYTES, false, false, move |context, _, retained| {
        let resources = retained.clone();
        let baseline = available(&resources.memory)?;
        let scene = scene(3, Extent::new(32, 16)?, Pixel::linear(1.0, 1.0, 1.0, 1.0)?, context)?;
        let mut preview = PreparePreview::new(
            scene,
            &resources,
            stamp(2, 1)?,
            stamp(2, 2)?,
            Extent::new(96, 96)?,
        )?;
        let remaining = available(&resources.memory)?;
        let pressure = resources.memory.claim(remaining - 1024)?;
        let error = preview
            .step(context)
            .err()
            .ok_or_else(|| WorkError::invalid("Over-budget candidate advanced"))?;
        assert!(contains_memory_error(&error));
        assert!(
            resources
                .engine
                .lock()
                .map_err(|_| WorkError::invalid("Active engine poisoned"))?
                .renderer
                .is_none()
        );
        drop(pressure);
        drop(preview);
        assert_eq!(available(&resources.memory)?, baseline);
        Ok(CpuStep::Complete(()))
    })
}
