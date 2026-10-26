// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify resumable preparation, publication completeness and superseded graph rejection.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_image_ops::{
    RasterSampleRequest, Reconstruction, validate_raster_sample_batch,
};

struct InterruptedSource(bool);
impl RasterSource for InterruptedSource {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if !self.0 {
            self.0 = true;
            return Err(ImageError::Source(Box::new(std::io::Error::from(
                std::io::ErrorKind::Interrupted,
            ))));
        }
        output.fill(Pixel::TRANSPARENT);
        Ok(())
    }
}

struct InterruptedBatchSource(Arc<AtomicU64>);
impl RasterSource for InterruptedBatchSource {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        output.fill(Pixel::TRANSPARENT);
        Ok(())
    }
    fn read_sample_batch(
        &mut self,
        _: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        validate_raster_sample_batch(requests)?;
        self.0.fetch_add(1, Ordering::Relaxed);
        requests[0].output.fill(Pixel::linear(1.0, 0.0, 0.0, 1.0)?);
        Err(ImageError::Source(Box::new(std::io::Error::from(std::io::ErrorKind::Interrupted))))
    }
}

#[test]
fn a_failed_sparse_batch_cannot_retry_into_a_published_update() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(4096, 2048)?;
        let target = Extent::new(256, 128)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(target)?;
        view.zoom_at(0.0625, [0.0; 2])?;
        let calls = Arc::new(AtomicU64::new(0));
        let mut renderer =
            CpuRenderer::new(InterruptedBatchSource(Arc::clone(&calls)), 64, context.memory())?;
        let mut preparation = renderer.begin(
            RenderRequest {
                document: &snapshot,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(20)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?;
        assert!(renderer.step(&mut preparation, context).is_err());
        for _ in 0..3 {
            assert!(renderer.step(&mut preparation, context).is_err());
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(renderer.stats().source_pixels, 0);
        assert_eq!(renderer.stats().computed_tiles, 0);
        Ok(())
    })
}

#[test]
fn a_failed_tile_cannot_be_skipped_to_publish_incomplete_patch_pixels() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(257, 131)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let view = Viewport::new(extent)?;
        let mut renderer = CpuRenderer::new(InterruptedSource(false), 64, context.memory())?;
        let mut preparation = renderer.begin(
            RenderRequest {
                document: &snapshot,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(extent),
            },
            context,
        )?;
        assert!(renderer.step(&mut preparation, context).is_err());
        for _ in 0..6 {
            assert!(renderer.step(&mut preparation, context).is_err());
        }
        Ok(())
    })
}

#[test]
fn cooperative_preparation_publishes_only_after_all_viewport_tiles_are_complete()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(333, 209)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let view = Viewport::new(extent)?;
        let mut renderer = CpuRenderer::new(
            Gradient { extent, constant: Some(Pixel::linear(0.2, 0.4, 0.6, 0.5)?) },
            64,
            context.memory(),
        )?;
        let mut preparation = renderer.begin(
            RenderRequest {
                document: &snapshot,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(extent),
            },
            context,
        )?;
        for _ in 0..5 {
            assert!(renderer.step(&mut preparation, context)?.is_none());
        }
        let update = renderer
            .step(&mut preparation, context)?
            .ok_or(RenderError::Invalid("Complete viewport did not publish"))?;
        assert!(renderer.step(&mut preparation, context).is_err());
        let surface = CpuSurface::from_prepared(update)?;
        let encoded =
            SrgbEncoder::new(context.memory())?.encode(Pixel::linear(0.1, 0.2, 0.3, 1.0)?)?;
        let expected = u32::from_be_bytes([255, encoded[0], encoded[1], encoded[2]]);
        assert!(surface.pixels().iter().all(|pixel| *pixel == expected));
        Ok(())
    })
}

#[test]
fn abandoned_sparse_preparation_releases_graph_tiles_without_invalidating_a_surface()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(4096, 2048)?;
        let target = Extent::new(256, 128)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(target)?;
        view.zoom_at(0.0625, [0.0; 2])?;
        let mut renderer = CpuRenderer::new(
            Gradient { extent, constant: Some(Pixel::linear(0.2, 0.4, 0.6, 1.0)?) },
            64,
            context.memory(),
        )?;
        let matte = black()?;
        let request = |target| RenderRequest {
            document: &snapshot,
            view,
            reconstruction: Reconstruction::Preview,
            matte,
            target,
            base: None,
            damage: DamagePlan::full(view.surface()),
        };
        let surface = CpuSurface::from_prepared(renderer.prepare(request(stamp(30)?), context)?)?;
        let retained_pixel = surface.pixels()[0];

        let mut abandoned = renderer.begin(request(stamp(31)?), context)?;
        assert!(renderer.step(&mut abandoned, context)?.is_none());
        assert!(renderer.stats().resident_tiles > 0);
        drop(abandoned);
        while renderer.evict_one() {}
        assert!(renderer.stats().resident_tiles > 0);
        let available_before = context
            .memory()
            .claim(u64::MAX)
            .err()
            .ok_or(RenderError::Invalid("Unbounded memory probe unexpectedly succeeded"))?
            .available_bytes;

        renderer.clear_graph_tiles();
        let available_after = context
            .memory()
            .claim(u64::MAX)
            .err()
            .ok_or(RenderError::Invalid("Unbounded memory probe unexpectedly succeeded"))?
            .available_bytes;
        assert_eq!(renderer.stats().resident_tiles, 0);
        assert!(available_after > available_before);
        assert_eq!(surface.pixels()[0], retained_pixel);
        Ok(())
    })
}

#[test]
fn replacing_the_compiled_graph_rejects_unpublished_earlier_preparations() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(257, 131)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let view = Viewport::new(extent)?;
        let matte = black()?;
        let target = stamp(1)?;
        let request = || RenderRequest {
            document: &snapshot,
            view,
            reconstruction: Reconstruction::Preview,
            matte,
            target,
            base: None,
            damage: DamagePlan::full(extent),
        };
        let mut renderer =
            CpuRenderer::new(Gradient { extent, constant: None }, 64, context.memory())?;
        let mut earlier = renderer.begin(request(), context)?;
        assert!(renderer.step(&mut earlier, context)?.is_none());
        let mut current = renderer.begin(request(), context)?;
        assert!(renderer.step(&mut earlier, context).is_err());
        assert!(renderer.step(&mut current, context)?.is_none());
        Ok(())
    })
}
