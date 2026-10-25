// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Upload bounded canonical source pixels, including DAG-addressed sparse decoder reads.
use super::*;
use sugarsubstitute_image_ops::AssetId;

impl<S: RasterSource> GpuEvaluator<S> {
    pub(super) fn source_tile(
        &mut self,
        asset: AssetId,
        region: Region,
        samples: Option<&RasterSamples>,
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
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
            }
            .map_err(image_error)?;
            let encoded = self.retry(|_| Encoding::pixels(region, &pixels, context))?;
            let tile = self.retry(|this| {
                let batch = this.ready(context)?;
                let buffer = batch.upload(
                    &encoded.bytes,
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    context,
                )?;
                Ok(GpuTile { buffer, region })
            })?;
            self.stats.source_pixels = self.stats.source_pixels.saturating_add(region.pixels());
            Computed {
                tile,
                coverage: Coverage::Known(pixels.iter().any(|pixel| pixel.channels()[3] > 0.0)),
            }
        })
    }
}
