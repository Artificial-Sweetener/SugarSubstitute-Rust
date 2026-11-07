// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep final sRGB8 display regions on the GPU while canonical graph tiles remain disposable.
use crate::{
    DisplayReadback, GpuError, GpuEvaluator,
    buffer::Buffer,
    display_readback::{DisplayReadbackWorkspace, read_buffer_region_into},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Extent, MAX_RASTER_SAMPLE_BATCH, NodeId, Region};

/// Immutable premultiplied sRGB8 device pixels, packed as little-endian `0xAARRGGBB` words.
/// Rows have 256-byte alignment for native texture copies; padding is excluded from readback.
/// Clones retain the selected GPU and the allocation's original execution charge.
#[derive(Clone)]
pub struct DisplayTile {
    pub(crate) buffer: Arc<Buffer>,
    pub(crate) region: Region,
}
/// Completed physical display regions in caller order, retaining their original inventory charge.
pub struct DisplayRegions {
    tiles: Vec<DisplayTile>,
    _memory: MemoryClaim,
}
impl DisplayRegions {
    /// Premultiplied device regions ready for presentation without CPU pixel conversion or upload.
    pub fn tiles(&self) -> &[DisplayTile] {
        &self.tiles
    }
}
pub(crate) const fn row_bytes(width: u32) -> u32 {
    (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
}
impl DisplayTile {
    /// Exact physical image region of this immutable device allocation.
    pub const fn region(&self) -> Region {
        self.region
    }
    /// Copy completed device pixels to an admitted worker for qualification or export.
    /// # Errors
    /// Preserves original native/control/admission causes and rejects non-premultiplied representations.
    pub fn read(&self, context: &JobContext) -> Result<DisplayReadback, GpuError> {
        let device = &self.buffer.device;
        let extent = Extent::new(self.region.right(), self.region.bottom())
            .map_err(crate::evaluate::image_error)?;
        let mut workspace =
            DisplayReadbackWorkspace::for_region(device, extent, self.region, context)?;
        read_buffer_region_into(self.buffer.clone(), self.region, &mut workspace, context)?;
        Ok(workspace.into_readback(self.region))
    }
}
impl<S> GpuEvaluator<S> {
    /// Evaluate up to 4096 regions and encode completed premultiplied sRGB8 pixels on the selected GPU.
    /// Canonical intermediates remain evictable; display rows use four-byte words with native alignment.
    /// Horizontal sparse-source siblings share one bounded source read before ordinary evaluation.
    /// Transfer bytes and nearest-16-bit sampling match the image-ops CPU sRGB encoder.
    /// # Errors
    /// Rejects excessive inventories and non-premultiplied output; preserves graph/control/native causes.
    pub fn display_regions(
        &mut self,
        node: NodeId,
        regions: &[Region],
        context: &JobContext,
    ) -> Result<DisplayRegions, GpuError> {
        self.cache.release_batch();
        if regions.len() > 4096 {
            return Err(GpuError::invalid("GPU display inventory exceeds its bound"));
        }
        let memory = context
            .memory()
            .claim((regions.len() * size_of::<DisplayTile>()) as u64 + 128)
            .map_err(|cause| GpuError::new("gpu_display_inventory", cause))?;
        let mut tiles = Vec::new();
        tiles
            .try_reserve_exact(regions.len())
            .map_err(|cause| GpuError::new("gpu_display_inventory_allocation", cause))?;
        if regions.is_empty() {
            self.settle(Ok(()), context)?;
            return Ok(DisplayRegions { tiles, _memory: memory });
        }
        self.cache.prefer_level(
            self.graph.requested_pyramid_level(node).map_err(crate::evaluate::image_error)?,
        );
        let sparse =
            self.graph.sparse_source_plan(node, context).map_err(crate::evaluate::image_error)?;
        let group_limit =
            sparse.maximum_regions(self.cache.capacity()).min(MAX_RASTER_SAMPLE_BATCH);
        let result = (|| {
            let mut start = 0;
            while start < regions.len() {
                let end = horizontal_group_end(regions, start, group_limit);
                self.cache.release_batch();
                self.prepare_sparse_regions(sparse, &regions[start..end], context)?;
                for region in &regions[start..end] {
                    let input = self.computed(node, *region, context)?.tile.clone();
                    let tile = self.retry(|this| {
                        this.ready(context)?;
                        let batch = this
                            .batch
                            .as_mut()
                            .ok_or_else(|| GpuError::invalid("GPU display batch is unavailable"))?;
                        this.kernels.display(batch, &input, context)
                    })?;
                    tiles.push(tile);
                }
                self.cache.release_batch();
                start = end;
            }
            Ok(())
        })();
        self.cache.release_batch();
        self.settle(result, context)?;
        Ok(DisplayRegions { tiles, _memory: memory })
    }
}

fn horizontal_group_end(regions: &[Region], start: usize, maximum: usize) -> usize {
    let first = regions[start];
    let mut end = start + 1;
    while end < regions.len() && end - start < maximum {
        let previous = regions[end - 1];
        let next = regions[end];
        if next.y() != first.y() || next.height() != first.height() || previous.right() > next.x() {
            break;
        }
        end += 1;
    }
    end
}
