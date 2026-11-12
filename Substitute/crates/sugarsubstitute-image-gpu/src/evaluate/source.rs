// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Upload bounded canonical source pixels, including DAG-addressed sparse decoder reads.
use super::*;
use sugarsubstitute_image_ops::{AssetId, CoverageSource};

impl<S> GpuEvaluator<S> {
    pub(super) fn source_tile(
        &mut self,
        node: NodeId,
        asset: AssetId,
        region: Region,
        samples: Option<&RasterSamples>,
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
        if let Some(binding) = self.source.device() {
            if samples.is_some()
                || binding.asset() != asset
                || self.graph.extent(node).map_err(image_error)? != binding.extent()
            {
                return Err(GpuError::invalid(
                    "Device raster does not admit sparse or foreign source reads",
                ));
            }
            let tile = binding.tile(region).cloned();
            let tile = match tile {
                Some(tile) => tile,
                None => self.gather(node, region, context)?,
            };
            // Conservatively keep work for device inputs; no CPU alpha probe or pixel readback.
            return Ok(Computed { tile, coverage: Coverage::Known(true) });
        }
        Ok({
            let _memory = context
                .memory()
                .claim(region.pixels() * 16 + 128)
                .map_err(|cause| GpuError::new("gpu_source_workspace", cause))?;
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(region.pixels() as usize)
                .map_err(|cause| GpuError::new("gpu_source_allocation", cause))?;
            pixels.resize(region.pixels() as usize, Pixel::TRANSPARENT);
            match samples {
                Some(samples) => self.source.read_samples(asset, samples, &mut pixels, context),
                None => self.source.read(asset, region, &mut pixels, context),
            }?;
            self.stats.source_pixels = self.stats.source_pixels.saturating_add(region.pixels());
            self.upload_pixels(region, &pixels, context)?
        })
    }

    /// Upload graph-retained coverage as premultiplied white whose alpha is the coverage,
    /// reading only the planned nearest samples of a sparse preview.
    pub(super) fn coverage_tile(
        &mut self,
        source: CoverageSource,
        region: Region,
        samples: Option<&RasterSamples>,
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
        let _memory = context
            .memory()
            .claim(region.pixels() * 16 + 128)
            .map_err(|cause| GpuError::new("gpu_coverage_workspace", cause))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(region.pixels() as usize)
            .map_err(|cause| GpuError::new("gpu_coverage_allocation", cause))?;
        pixels.resize(region.pixels() as usize, Pixel::TRANSPARENT);
        self.graph
            .coverage_source(source)
            .and_then(|coverage| match samples {
                Some(samples) => coverage.read_samples(samples, &mut pixels),
                None => coverage.read_pixels(region, &mut pixels),
            })
            .map_err(image_error)?;
        self.upload_pixels(region, &pixels, context)
    }

    fn upload_pixels(
        &mut self,
        region: Region,
        pixels: &[Pixel],
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
        Ok({
            let encoded = self.retry(|_| Encoding::pixels(region, pixels, context))?;
            let tile = self.retry(|this| {
                let batch = this.ready(context)?;
                let buffer = batch.upload(
                    &encoded.bytes,
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    context,
                )?;
                Ok(GpuTile { buffer, region })
            })?;
            Computed {
                tile,
                coverage: Coverage::Known(pixels.iter().any(|pixel| pixel.channels()[3] > 0.0)),
            }
        })
    }
}
