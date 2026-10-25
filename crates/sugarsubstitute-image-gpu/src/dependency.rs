// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Assemble bounded GPU halos with device copies, preserving aligned source and pyramid reuse.
use crate::{GpuError, GpuEvaluator, GpuTile, evaluate::image_error};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{NodeId, Operation, RasterSource, Region, Sampling, TILE_EDGE};

impl<S: RasterSource> GpuEvaluator<S> {
    pub(crate) fn input(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<GpuTile, GpuError> {
        let aligned = match self.graph.operation(node).map_err(image_error)? {
            Operation::Raster { .. } => true,
            Operation::Resample { input, extent, sampling } => {
                sampling == Sampling::resize(self.graph.extent(input).map_err(image_error)?, extent)
            }
            _ => false,
        };
        if !aligned {
            return Ok(self.computed(node, region, context)?.tile.clone());
        }
        let x = region.x() / TILE_EDGE;
        let y = region.y() / TILE_EDGE;
        if self.graph.extent(node).map_err(image_error)?.tile_region(x, y).map_err(image_error)?
            == region
        {
            return Ok(self.computed(node, region, context)?.tile.clone());
        }
        self.gather(node, region, context)
    }
    pub(crate) fn gather(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<GpuTile, GpuError> {
        if region.width() > 2 * TILE_EDGE + 16 || region.height() > 2 * TILE_EDGE + 16 {
            return Err(GpuError::invalid("GPU dependency exceeds bounded tile workspace"));
        }
        let storage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let output =
            self.retry(|this| this.device.allocate(region.pixels() * 16, storage, context))?;
        for y in region.y() / TILE_EDGE..=(region.bottom() - 1) / TILE_EDGE {
            for x in region.x() / TILE_EDGE..=(region.right() - 1) / TILE_EDGE {
                let tile_region = self
                    .graph
                    .extent(node)
                    .map_err(image_error)?
                    .tile_region(x, y)
                    .map_err(image_error)?;
                let tile = self.computed(node, tile_region, context)?.tile.clone();
                let overlap = tile
                    .region()
                    .intersection(region)
                    .ok_or_else(|| GpuError::invalid("Missing GPU dependency intersection"))?;
                let batch = self.ready(context)?;
                batch.pin(tile.buffer.clone())?;
                batch.pin(output.clone())?;
                let commands = batch.commands()?;
                for row in overlap.y()..overlap.bottom() {
                    let source = (u64::from(row - tile.region().y())
                        * u64::from(tile.region().width())
                        + u64::from(overlap.x() - tile.region().x()))
                        * 16;
                    let target = (u64::from(row - region.y()) * u64::from(region.width())
                        + u64::from(overlap.x() - region.x()))
                        * 16;
                    commands.copy_buffer_to_buffer(
                        &tile.buffer.raw,
                        source,
                        &output.raw,
                        target,
                        u64::from(overlap.width()) * 16,
                    );
                }
                batch.copied();
                crate::adapter::check(context)?;
            }
        }
        Ok(GpuTile { buffer: output, region })
    }
}
