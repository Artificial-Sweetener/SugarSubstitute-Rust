// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Direct previews stay sparse while finished reconstruction retains canonical composition.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_image_ops::{
    Filter, RasterSampleRequest, Reconstruction, validate_raster_sample_batch,
};

#[derive(Default)]
struct ReadCounts {
    broad_pixels: AtomicU64,
    sparse_pixels: AtomicU64,
    scalar_sparse_reads: AtomicU64,
    batches: AtomicU64,
}

#[test]
fn navigation_uses_the_nearest_ready_coarser_pyramid_before_required_fine_reconstruction()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(1024, 1024)?;
        let target = Extent::new(128, 128)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(target)?;
        view.fit(extent)?;
        let counts = Arc::new(ReadCounts::default());
        let mut renderer = CpuRenderer::new(
            CountedSource {
                extent,
                color: Pixel::linear(0.2, 0.4, 0.6, 1.0)?,
                counts: counts.clone(),
            },
            16,
            context.memory(),
        )?;
        let matte = black()?;
        let request = |view, reconstruction, target| RenderRequest {
            document: &snapshot,
            comparison: None,
            view,
            reconstruction,
            matte,
            target,
            base: None,
            damage: DamagePlan::full(view.surface()),
        };
        drop(renderer.prepare(
            request(view, Reconstruction::Finished(Filter::Lanczos3), stamp(1)?),
            context,
        )?);
        while renderer.stats().resident_tiles > 1 {
            assert!(renderer.evict_one());
        }
        let reads = counts.broad_pixels.load(Ordering::Relaxed);
        view.zoom_at(2.0, [64.0, 64.0])?;
        let mut navigation = renderer.begin_navigation(
            request(view, Reconstruction::navigation_for_display_zoom(view.zoom())?, stamp(2)?),
            context,
        )?;
        assert_eq!(navigation.sampling_scale(), 0.125);
        loop {
            if let Some(update) = renderer.step(&mut navigation, context)? {
                drop(update);
                break;
            }
        }
        assert_eq!(counts.broad_pixels.load(Ordering::Relaxed), reads);
        drop(renderer.prepare(
            request(view, Reconstruction::Finished(Filter::Lanczos3), stamp(3)?),
            context,
        )?);
        assert!(counts.broad_pixels.load(Ordering::Relaxed) > reads);
        Ok(())
    })
}

#[test]
fn bilinear_navigation_reuses_ready_canonical_pyramid_tiles_without_reading_source_again()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(512, 512)?;
        let target = Extent::new(256, 256)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(target)?;
        view.fit(extent)?;
        let counts = Arc::new(ReadCounts::default());
        let mut renderer = CpuRenderer::new(
            CountedSource {
                extent,
                color: Pixel::linear(0.2, 0.4, 0.6, 1.0)?,
                counts: counts.clone(),
            },
            256,
            context.memory(),
        )?;
        let prepare = |renderer: &mut CpuRenderer<CountedSource>, view, reconstruction, target| {
            renderer.prepare(
                RenderRequest {
                    document: &snapshot,
                    comparison: None,
                    view,
                    reconstruction,
                    matte: black()?,
                    target,
                    base: None,
                    damage: DamagePlan::full(view.surface()),
                },
                context,
            )
        };
        let warm =
            prepare(&mut renderer, view, Reconstruction::Finished(Filter::Lanczos3), stamp(1)?)?;
        drop(warm);
        let before = renderer.stats();
        let reads = counts.broad_pixels.load(Ordering::Relaxed);
        view.zoom_at(0.8, [128.0, 128.0])?;
        let navigation = prepare(
            &mut renderer,
            view,
            Reconstruction::navigation_for_display_zoom(view.zoom())?,
            stamp(2)?,
        )?;
        assert_eq!(renderer.stats().source_pixels, before.source_pixels);
        assert_eq!(counts.broad_pixels.load(Ordering::Relaxed), reads);
        assert_eq!(counts.sparse_pixels.load(Ordering::Relaxed), 0);
        assert!(renderer.stats().reused_tiles > before.reused_tiles);
        let surface = CpuSurface::from_prepared(navigation)?;
        assert_eq!(surface.extent(), target);
        Ok(())
    })
}

struct CountedSource {
    extent: Extent,
    color: Pixel,
    counts: Arc<ReadCounts>,
}

impl RasterSource for CountedSource {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != AssetId([1; 32]) || region.intersection(self.extent.region()) != Some(region) {
            return Err(ImageError::Invalid("Invalid counted raster request"));
        }
        self.counts.broad_pixels.fetch_add(region.pixels(), Ordering::Relaxed);
        output.fill(self.color);
        Ok(())
    }

    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &sugarsubstitute_image_ops::RasterSamples,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != AssetId([1; 32])
            || samples.source() != self.extent
            || output.len() as u64 != samples.region().pixels()
        {
            return Err(ImageError::Invalid("Invalid counted sparse raster request"));
        }
        self.counts.scalar_sparse_reads.fetch_add(1, Ordering::Relaxed);
        self.counts.sparse_pixels.fetch_add(samples.region().pixels(), Ordering::Relaxed);
        output.fill(self.color);
        Ok(())
    }

    fn read_sample_batch(
        &mut self,
        asset: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        validate_raster_sample_batch(requests)?;
        if asset != AssetId([1; 32]) {
            return Err(ImageError::Invalid("Invalid counted sparse batch asset"));
        }
        self.counts.batches.fetch_add(1, Ordering::Relaxed);
        for request in requests {
            self.counts
                .sparse_pixels
                .fetch_add(request.samples.region().pixels(), Ordering::Relaxed);
            request.output.fill(self.color);
        }
        Ok(())
    }
}

struct ScalarSource {
    extent: Extent,
    color: Pixel,
}
impl RasterSource for ScalarSource {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != AssetId([1; 32]) || region.intersection(self.extent.region()) != Some(region) {
            return Err(ImageError::Invalid("Invalid scalar raster request"));
        }
        output.fill(self.color);
        Ok(())
    }
    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &sugarsubstitute_image_ops::RasterSamples,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != AssetId([1; 32])
            || samples.source() != self.extent
            || output.len() as u64 != samples.region().pixels()
        {
            return Err(ImageError::Invalid("Invalid scalar sparse raster request"));
        }
        output.fill(self.color);
        Ok(())
    }
}

#[test]
fn preview_reads_only_sparse_source_pixels_while_finished_uses_lanczos_reduction()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(257, 193)?;
        let target = Extent::new(16, 12)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(target)?;
        view.zoom_at(0.0625, [0.0; 2])?;
        let color = Pixel::linear(0.2, 0.4, 0.6, 1.0)?;

        let preview_counts = Arc::new(ReadCounts::default());
        let mut preview_renderer = CpuRenderer::new(
            CountedSource { extent, color, counts: Arc::clone(&preview_counts) },
            64,
            context.memory(),
        )?;
        let preview = CpuSurface::from_prepared(preview_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?)?;
        assert_eq!(preview_counts.broad_pixels.load(Ordering::Relaxed), 0);
        assert_eq!(preview_counts.sparse_pixels.load(Ordering::Relaxed), target.pixels());
        assert_eq!(preview_counts.scalar_sparse_reads.load(Ordering::Relaxed), 1);
        assert_eq!(preview_counts.batches.load(Ordering::Relaxed), 0);
        assert_eq!(preview_renderer.stats().source_pixels, target.pixels());

        let finished_counts = Arc::new(ReadCounts::default());
        let mut finished_renderer = CpuRenderer::new(
            CountedSource { extent, color, counts: Arc::clone(&finished_counts) },
            64,
            context.memory(),
        )?;
        let finished = CpuSurface::from_prepared(finished_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                matte: black()?,
                target: stamp(2)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?)?;
        assert_eq!(finished_counts.sparse_pixels.load(Ordering::Relaxed), 0);
        assert!(finished_counts.broad_pixels.load(Ordering::Relaxed) > target.pixels());
        let encoded = SrgbEncoder::new(context.memory())?.encode(color)?;
        let expected = u32::from_be_bytes([255, encoded[0], encoded[1], encoded[2]]);
        assert!(preview.pixels().iter().all(|pixel| *pixel == expected));
        assert_eq!(finished.pixels(), preview.pixels());
        Ok(())
    })
}

#[test]
fn preview_batch_pins_survive_a_full_finished_pyramid_cache() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(1024, 512)?;
        let target = Extent::new(256, 128)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(target)?;
        view.zoom_at(0.25, [0.0; 2])?;
        let counts = Arc::new(ReadCounts::default());
        let mut renderer = CpuRenderer::new(
            CountedSource {
                extent,
                color: Pixel::linear(0.15, 0.4, 0.7, 1.0)?,
                counts: Arc::clone(&counts),
            },
            16,
            context.memory(),
        )?;
        drop(renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                matte: black()?,
                target: stamp(3)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?);
        assert_eq!(renderer.stats().resident_tiles, 16);

        let mut preview = renderer.begin(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(4)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?;
        assert!(renderer.step(&mut preview, context)?.is_none());
        assert!(renderer.step(&mut preview, context)?.is_some());
        assert_eq!(counts.batches.load(Ordering::Relaxed), 1);
        assert_eq!(counts.scalar_sparse_reads.load(Ordering::Relaxed), 0);
        assert_eq!(counts.sparse_pixels.load(Ordering::Relaxed), target.pixels());
        assert!(renderer.stats().resident_tiles <= 16);
        Ok(())
    })
}

#[test]
fn horizontal_preview_batch_matches_scalar_mixed_composition() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(4096, 2048)?;
        let target = Extent::new(256, 128)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let vector =
            VectorRectangle::new([0.0, 0.0, 4096.0, 2048.0], Pixel::linear(0.7, 0.15, 0.05, 1.0)?)?;
        document.apply(DocumentEdit::Insert {
            index: 0,
            layer: Layer::new(LayerId::new(1)?, LayerContent::VectorRectangle(vector))?,
        })?;
        let adjustment_mask =
            VectorRectangle::new([0.0, 0.0, 2112.0, 2048.0], Pixel::linear(0.0, 0.0, 0.0, 0.65)?)?;
        let adjustment = Layer::new(LayerId::new(2)?, LayerContent::Exposure { stops: 0.75 })?
            .with_opacity(0.55)?
            .with_mask(Some(LayerMask::VectorRectangle(adjustment_mask)));
        document.apply(DocumentEdit::Insert { index: 1, layer: adjustment })?;
        let raster =
            Layer::new(LayerId::new(3)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
                .with_opacity(0.7)?;
        document.apply(DocumentEdit::Insert { index: 2, layer: raster })?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(target)?;
        view.zoom_at(0.0625, [0.0; 2])?;
        let color = Pixel::linear(0.1, 0.35, 0.8, 0.5)?;
        let counts = Arc::new(ReadCounts::default());
        let mut batched_renderer = CpuRenderer::new(
            CountedSource { extent, color, counts: Arc::clone(&counts) },
            64,
            context.memory(),
        )?;
        let batched = CpuSurface::from_prepared(batched_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(10)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?)?;
        assert_eq!(counts.batches.load(Ordering::Relaxed), 1);
        assert_eq!(counts.broad_pixels.load(Ordering::Relaxed), 0);
        assert_eq!(counts.sparse_pixels.load(Ordering::Relaxed), target.pixels());

        let mut scalar_renderer =
            CpuRenderer::new(ScalarSource { extent, color }, 64, context.memory())?;
        let scalar = CpuSurface::from_prepared(scalar_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(11)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?)?;
        assert_eq!(batched.pixels(), scalar.pixels());
        Ok(())
    })
}

#[test]
fn transparent_authored_mask_keeps_preview_source_lazy() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(4096, 2048)?;
        let target = Extent::new(256, 128)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let mask = VectorRectangle::new([0.0, 0.0, 4096.0, 2048.0], Pixel::TRANSPARENT)?;
        let raster =
            Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
                .with_mask(Some(LayerMask::VectorRectangle(mask)));
        document.apply(DocumentEdit::Insert { index: 0, layer: raster })?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(target)?;
        view.zoom_at(0.0625, [0.0; 2])?;
        let counts = Arc::new(ReadCounts::default());
        let mut renderer = CpuRenderer::new(
            CountedSource {
                extent,
                color: Pixel::linear(1.0, 0.0, 0.0, 1.0)?,
                counts: Arc::clone(&counts),
            },
            64,
            context.memory(),
        )?;
        let surface = CpuSurface::from_prepared(renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Preview,
                matte: black()?,
                target: stamp(12)?,
                base: None,
                damage: DamagePlan::full(target),
            },
            context,
        )?)?;
        assert_eq!(counts.batches.load(Ordering::Relaxed), 0);
        assert_eq!(counts.broad_pixels.load(Ordering::Relaxed), 0);
        assert_eq!(counts.sparse_pixels.load(Ordering::Relaxed), 0);
        assert!(surface.pixels().iter().all(|pixel| *pixel == 0xff000000));
        Ok(())
    })
}
