// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prefill horizontal sparse source leaves through one storage-locality batch.

use super::*;
use sugarsubstitute_contracts::execution::MemoryClaim;
use sugarsubstitute_image_ops::{MAX_RASTER_SAMPLE_BATCH, RasterSampleRequest, SparseSourcePlan};

struct StagedSparseTile {
    samples: RasterSamples,
    pixels: Vec<Pixel>,
    _memory: MemoryClaim,
}

struct UploadedSparseTile {
    region: Region,
    computed: Arc<Computed>,
}

impl<S: RasterSource> GpuEvaluator<S> {
    /// Read and upload only uncached mandatory source leaves for one horizontal output group.
    pub(crate) fn prepare_sparse_regions(
        &mut self,
        plan: SparseSourcePlan,
        regions: &[Region],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        if regions.len() <= 1 {
            return Ok(());
        }
        crate::adapter::check(context)?;
        let first = regions[0];
        if regions.len() > MAX_RASTER_SAMPLE_BATCH
            || regions.iter().any(|region| {
                region.y() != first.y()
                    || region.height() != first.height()
                    || region.width() > TILE_EDGE
                    || region.height() > TILE_EDGE
            })
            || regions.windows(2).any(|pair| pair[0].right() > pair[1].x())
            || regions.len() > plan.maximum_regions(self.cache.capacity())
        {
            return Err(GpuError::invalid("Invalid sparse GPU output region batch"));
        }
        let Some(source) = plan.source() else {
            return Ok(());
        };
        let key = self.graph.fingerprint(source).map_err(image_error)?;
        let level = self.graph.pyramid_level(source).map_err(image_error)?;
        let metadata_bytes = regions
            .len()
            .checked_mul(
                size_of::<StagedSparseTile>()
                    + size_of::<UploadedSparseTile>()
                    + size_of::<RasterSampleRequest<'_>>(),
            )
            .and_then(|bytes| bytes.checked_add(128))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| GpuError::invalid("Sparse GPU metadata exceeds its bound"))?;
        let _metadata = context
            .memory()
            .claim(metadata_bytes)
            .map_err(|cause| GpuError::new("gpu_sparse_metadata", cause))?;
        let mut staged = Vec::new();
        staged
            .try_reserve_exact(regions.len())
            .map_err(|cause| GpuError::new("gpu_sparse_metadata_allocation", cause))?;
        let mut asset = None;
        for &region in regions {
            if let Some(computed) = self.cache.find(key, region, level) {
                self.cache.insert_for_batch(key, region, level, computed).map_err(image_error)?;
                continue;
            }
            let Some((current, samples)) =
                plan.samples(&self.graph, region).map_err(image_error)?
            else {
                continue;
            };
            if asset.is_some_and(|asset| asset != current) {
                return Err(GpuError::invalid("Sparse GPU batch mixed raster assets"));
            }
            asset = Some(current);
            let workspace_bytes = samples
                .region()
                .pixels()
                .checked_mul(size_of::<Pixel>() as u64)
                .and_then(|bytes| bytes.checked_add(128))
                .ok_or_else(|| GpuError::invalid("Sparse GPU workspace exceeds its bound"))?;
            let memory = context
                .memory()
                .claim(workspace_bytes)
                .map_err(|cause| GpuError::new("gpu_sparse_workspace", cause))?;
            let pixel_count = samples.region().pixels() as usize;
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(pixel_count)
                .map_err(|cause| GpuError::new("gpu_sparse_pixel_allocation", cause))?;
            pixels.resize(pixel_count, Pixel::TRANSPARENT);
            staged.push(StagedSparseTile { samples, pixels, _memory: memory });
        }
        if staged.len() <= 1 {
            return Ok(());
        }
        let asset = asset.ok_or_else(|| GpuError::invalid("Sparse GPU batch lost its asset"))?;
        let mut requests = Vec::new();
        requests
            .try_reserve_exact(staged.len())
            .map_err(|cause| GpuError::new("gpu_sparse_request_allocation", cause))?;
        requests.extend(staged.iter_mut().map(|staged| RasterSampleRequest {
            samples: &staged.samples,
            output: &mut staged.pixels,
        }));
        self.source.read_sample_batch(asset, &mut requests, context).map_err(image_error)?;
        crate::adapter::check(context)?;
        drop(requests);

        let mut uploaded = Vec::new();
        uploaded
            .try_reserve_exact(staged.len())
            .map_err(|cause| GpuError::new("gpu_sparse_upload_inventory", cause))?;
        for staged in &staged {
            let region = staged.samples.region();
            let encoded = self.retry(|_| Encoding::pixels(region, &staged.pixels, context))?;
            let tile = self.retry(|this| {
                let batch = this.ready(context)?;
                let buffer = batch.upload(
                    &encoded.bytes,
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    context,
                )?;
                Ok(GpuTile { buffer, region })
            })?;
            uploaded.push(UploadedSparseTile {
                region,
                computed: Arc::new(Computed {
                    tile,
                    coverage: Coverage::Known(
                        staged.pixels.iter().any(|pixel| pixel.channels()[3] > 0.0),
                    ),
                }),
            });
        }
        crate::adapter::check(context)?;
        for uploaded in uploaded {
            self.cache
                .insert_for_batch(key, uploaded.region, level, uploaded.computed)
                .map_err(image_error)?;
            self.stats.source_pixels =
                self.stats.source_pixels.saturating_add(uploaded.region.pixels());
            self.stats.computed_tiles = self.stats.computed_tiles.saturating_add(1);
            self.stats.computed_pixels =
                self.stats.computed_pixels.saturating_add(uploaded.region.pixels());
        }
        Ok(())
    }
}
