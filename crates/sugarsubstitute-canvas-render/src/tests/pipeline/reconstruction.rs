// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Direct previews stay sparse while finished reconstruction retains canonical composition.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_image_ops::{Filter, Reconstruction};

#[derive(Default)]
struct ReadCounts {
    broad_pixels: AtomicU64,
    sparse_pixels: AtomicU64,
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
        self.counts.sparse_pixels.fetch_add(samples.region().pixels(), Ordering::Relaxed);
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
fn minified_nearest_and_lanczos_share_canonical_translucent_composite() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(64, 64)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let layer =
            Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
                .with_opacity(0.75)?
                .with_mask(Some(LayerMask::Raster { asset: AssetId([1; 32]), extent }));
        document.apply(DocumentEdit::Insert { index: 0, layer })?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(Extent::new(8, 8)?)?;
        view.zoom_at(0.125, [0.0; 2])?;
        let source = Pixel::linear(0.8, 0.2, 0.4, 0.5)?;
        let mut nearest_renderer =
            CpuRenderer::new(Gradient { extent, constant: Some(source) }, 64, context.memory())?;
        let nearest = CpuSurface::from_prepared(nearest_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                reconstruction: Reconstruction::Finished(Filter::Nearest),
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?)?;
        let mut lanczos_renderer =
            CpuRenderer::new(Gradient { extent, constant: Some(source) }, 64, context.memory())?;
        let lanczos = CpuSurface::from_prepared(lanczos_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                matte: black()?,
                target: stamp(2)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?)?;
        assert_eq!(nearest.pixels(), lanczos.pixels());
        let expected =
            SrgbEncoder::new(context.memory())?.encode(Pixel::linear(0.15, 0.0375, 0.075, 1.0)?)?;
        let expected = u32::from_be_bytes([255, expected[0], expected[1], expected[2]]);
        assert!(nearest.pixels().iter().all(|pixel| *pixel == expected));
        Ok(())
    })
}
