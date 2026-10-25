// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep final sRGB8 display regions on the GPU while canonical graph tiles remain disposable.
use crate::{
    GpuError, GpuEvaluator,
    buffer::Buffer,
    transfer::{Mapping, Transfer},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{NodeId, RasterSource, Region};

/// Immutable opaque sRGB8 device pixels, packed as little-endian `0xffRRGGBB` words.
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
    /// Opaque device regions ready for presentation without CPU pixel conversion or upload.
    pub fn tiles(&self) -> &[DisplayTile] {
        &self.tiles
    }
}
/// Native display-pixel readback for export or headless qualification; presentation stays on GPU.
pub struct DisplayReadback {
    region: Region,
    words: Vec<u32>,
    _memory: MemoryClaim,
}
impl DisplayReadback {
    /// Exact device-pixel coordinates represented by these row-major words.
    pub const fn region(&self) -> Region {
        self.region
    }
    /// Opaque sRGB words in `0xffRRGGBB` channel order, independent of host byte order.
    pub fn pixels(&self) -> &[u32] {
        &self.words
    }
}
impl DisplayTile {
    /// Exact physical image region of this immutable device allocation.
    pub const fn region(&self) -> Region {
        self.region
    }
    /// Copy completed device pixels to an admitted worker for qualification or export.
    /// # Errors
    /// Preserves original native/control/admission causes and rejects nonopaque representations.
    pub fn read(&self, context: &JobContext) -> Result<DisplayReadback, GpuError> {
        let memory = context
            .memory()
            .claim(self.region.pixels() * 4 + 128)
            .map_err(|cause| GpuError::new("gpu_display_readback", cause))?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(self.region.pixels() as usize)
            .map_err(|cause| GpuError::new("gpu_display_readback_allocation", cause))?;
        let device = &self.buffer.device;
        let receiving = device.allocate(
            self.buffer.bytes,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        Transfer::copy(self.buffer.clone(), receiving.clone()).retire()?;
        crate::adapter::check(context)?;
        let mut mapping = Mapping::new(receiving);
        mapping.wait()?;
        {
            let view = mapping.view()?;
            for bytes in view.chunks_exact(4).take(self.region.pixels() as usize) {
                let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if value >> 24 != 255 {
                    return Err(GpuError::invalid("GPU display result is not opaque"));
                }
                words.push(value);
            }
        }
        mapping.finish()?;
        crate::adapter::check(context)?;
        Ok(DisplayReadback { region: self.region, words, _memory: memory })
    }
}
impl<S: RasterSource> GpuEvaluator<S> {
    /// Evaluate up to 4096 regions and encode completed opaque sRGB8 pixels on the selected GPU.
    /// Canonical intermediates remain evictable; retained final data uses four bytes per pixel.
    /// Transfer bytes and nearest-16-bit sampling match the image-ops CPU sRGB encoder.
    /// # Errors
    /// Rejects excessive inventories and nonopaque output; preserves graph/control/native causes.
    pub fn display_regions(
        &mut self,
        node: NodeId,
        regions: &[Region],
        context: &JobContext,
    ) -> Result<DisplayRegions, GpuError> {
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
        let result = (|| {
            for region in regions {
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
            Ok(())
        })();
        self.settle(result, context)?;
        Ok(DisplayRegions { tiles, _memory: memory })
    }
}
