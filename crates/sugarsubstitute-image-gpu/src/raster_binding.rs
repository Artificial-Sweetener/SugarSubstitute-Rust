// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind a complete bounded local canonical raster to one explicitly selected device.
use crate::{GpuDevice, GpuError, GpuEvaluator, GpuTile, evaluate::image_error};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::{
    AssetId, Extent, Filter, Graph, NodeId, Operation, Region, TILE_EDGE, TileRetention,
};

/// Immutable complete local raster; inputs retain their original device allocation charges.
/// Source tiles remain available independently of disposable computed-tile cache eviction.
pub struct GpuRasterBinding {
    asset: AssetId,
    extent: Extent,
    device: Arc<GpuDevice>,
    tiles: Vec<GpuTile>,
    _memory: MemoryClaim,
}
impl GpuRasterBinding {
    /// Admit every clipped row-major 128-pixel source tile exactly once.
    /// The caller owns immutable asset identity; reusing it for changed pixels is invalid.
    /// `retained` charges inventory metadata for the binding lifetime; `context` supplies controls.
    /// # Errors
    /// Rejects incomplete, overlapping, reordered, foreign-device or excessive source inventories.
    pub fn from_tiles(
        asset: AssetId,
        extent: Extent,
        tiles: Vec<GpuTile>,
        retained: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        validate_layout(extent, tiles.iter().map(GpuTile::region))?;
        let device = tiles
            .first()
            .ok_or_else(|| GpuError::invalid("Device raster is empty"))?
            .buffer
            .device
            .clone();
        if tiles.iter().any(|tile| !Arc::ptr_eq(&device, &tile.buffer.device)) {
            return Err(GpuError::invalid("Device raster mixes selected GPUs"));
        }
        let memory = retained
            .claim(tiles.capacity() as u64 * size_of::<GpuTile>() as u64 + 256)
            .map_err(|cause| GpuError::new("gpu_raster_inventory", cause))?;
        Ok(Self { asset, extent, device, tiles, _memory: memory })
    }
    /// Immutable image-operation source identity.
    pub const fn asset(&self) -> AssetId {
        self.asset
    }
    /// Local source dimensions; no proportional full-viewport allocation is implied.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    pub(crate) fn tile(&self, region: Region) -> Option<&GpuTile> {
        let columns = self.extent.width().div_ceil(TILE_EDGE);
        let index = region.y() / TILE_EDGE * columns + region.x() / TILE_EDGE;
        self.tiles.get(index as usize).filter(|tile| tile.region() == region)
    }
}

fn validate_layout(
    extent: Extent,
    regions: impl IntoIterator<Item = Region>,
) -> Result<(), GpuError> {
    if extent.width() > 16384 || extent.height() > 16384 {
        return Err(GpuError::invalid("Device raster extent exceeds its bound"));
    }
    let columns = extent.width().div_ceil(TILE_EDGE);
    let rows = extent.height().div_ceil(TILE_EDGE);
    let count = columns as usize * rows as usize;
    if count > 4096 {
        return Err(GpuError::invalid("Device raster exceeds its tile bound"));
    }
    let mut actual = regions.into_iter();
    for y in 0..rows {
        for x in 0..columns {
            if actual.next() != Some(extent.tile_region(x, y).map_err(image_error)?) {
                return Err(GpuError::invalid(
                    "Device raster does not completely cover its local extent",
                ));
            }
        }
    }
    if actual.next().is_some() {
        return Err(GpuError::invalid("Device raster has extra tiles"));
    }
    Ok(())
}

impl GpuEvaluator<()> {
    /// Bound reuse metadata by the complete local unary graph's canonical tile inventories.
    /// This is not a pixel allocation; native tiles remain demand-evaluated and evictable.
    /// The existing 4096-entry retention limit also bounds graphs with large local crops.
    /// # Errors
    /// Rejects unsupported or overdeep device-raster graphs before any native work.
    pub fn raster_tile_capacity(graph: &Graph, mut node: NodeId) -> Result<usize, GpuError> {
        let mut tiles = 0_u64;
        for _ in 0..128 {
            tiles =
                (tiles + graph.extent(node).map_err(image_error)?.region().tile_count()).min(4096);
            match graph.operation(node).map_err(image_error)? {
                Operation::Raster { .. } => return Ok(tiles as usize),
                Operation::GaussianBlur { input, .. } | Operation::Exposure { input, .. } => {
                    node = input;
                }
                Operation::Resample { input, sampling, .. }
                    if sampling.filter() != Filter::Nearest =>
                {
                    node = input;
                }
                _ => return Err(GpuError::invalid("Unsupported device-raster retention graph")),
            }
        }
        Err(GpuError::invalid("Device-raster retention graph exceeds its depth"))
    }
    /// Grow disposable cache metadata for a later local crop without recompiling GPU kernels.
    /// Existing cached aliases are released when growth is necessary; outside results stay charged.
    /// # Errors
    /// Rejects pending work, excessive capacity, or failed retained metadata admission.
    pub fn reserve_raster_tiles(
        &mut self,
        maximum: usize,
        memory: &MemoryLease,
    ) -> Result<(), GpuError> {
        if self.batch.is_some() {
            return Err(GpuError::invalid("Device-raster retention has pending work"));
        }
        if maximum > self.cache.capacity() {
            self.cache = TileRetention::new(maximum, memory).map_err(image_error)?;
        }
        Ok(())
    }
    /// Evaluate a unary filtered presentation graph from complete immutable device pixels.
    /// No host decoder or CPU pixel-upload path exists in this source mode.
    /// # Errors
    /// Rejects foreign assets/devices/extents and unsupported graph operations before GPU work.
    pub fn from_raster(
        graph: Graph,
        output: NodeId,
        binding: GpuRasterBinding,
        maximum_tiles: usize,
        device: Arc<GpuDevice>,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        validate_graph(&graph, output, binding.asset, binding.extent)?;
        if !Arc::ptr_eq(&device, &binding.device) {
            return Err(GpuError::invalid("Bound raster belongs to a different selected GPU"));
        }
        Self::from_input(
            graph,
            crate::raster_input::RasterInput::Device { binding },
            maximum_tiles,
            device,
            memory,
            context,
        )
    }
    /// Replace a completed presentation source without retaining stale cache content or GPU work.
    /// # Errors
    /// Rejects pending batches and invalid/foreign source graphs before changing this evaluator.
    pub fn replace_raster(
        &mut self,
        graph: Graph,
        output: NodeId,
        binding: GpuRasterBinding,
    ) -> Result<(), GpuError> {
        validate_graph(&graph, output, binding.asset, binding.extent)?;
        if self.batch.is_some() || !Arc::ptr_eq(&self.device, &binding.device) {
            return Err(GpuError::invalid(
                "Device raster replacement has foreign or pending custody",
            ));
        }
        self.clear_tiles();
        self.graph = graph;
        self.source = crate::raster_input::RasterInput::Device { binding };
        Ok(())
    }
}
fn validate_graph(
    graph: &Graph,
    mut node: NodeId,
    asset: AssetId,
    extent: Extent,
) -> Result<NodeId, GpuError> {
    for _ in 0..128 {
        match graph.operation(node).map_err(image_error)? {
            Operation::Raster { asset: actual, extent: actual_extent }
                if actual == asset && actual_extent == extent =>
            {
                return Ok(node);
            }
            Operation::GaussianBlur { input, .. } | Operation::Exposure { input, .. } => {
                node = input
            }
            Operation::Resample { input, sampling, .. } if sampling.filter() != Filter::Nearest => {
                node = input
            }
            _ => {
                return Err(GpuError::invalid(
                    "Bound raster requires a matching unary filtered graph",
                ));
            }
        }
    }
    Err(GpuError::invalid("Bound raster graph exceeds its depth"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_inventory_requires_every_exact_clipped_tile_once() -> Result<(), GpuError> {
        let extent = Extent::new(259, 133).map_err(image_error)?;
        let regions: Vec<_> = (0..2)
            .flat_map(|y| (0..3).map(move |x| extent.tile_region(x, y)))
            .collect::<Result<_, _>>()
            .map_err(image_error)?;
        validate_layout(extent, regions.iter().copied())?;
        assert!(validate_layout(extent, regions[..5].iter().copied()).is_err());
        assert!(validate_layout(extent, regions.iter().rev().copied()).is_err());
        assert!(
            validate_layout(extent, regions.iter().copied().chain(regions[..1].iter().copied()))
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn bound_graph_rejects_foreign_assets_and_sparse_nearest_before_native_work()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Release;
        impl sugarsubstitute_contracts::execution::MemoryRelease for Release {
            fn release(&self, _: u64) {}
        }
        let memory = MemoryLease::new(1024 * 1024, Box::new(Release));
        let extent = Extent::new(9, 7)?;
        let asset = AssetId([1; 32]);
        let mut graph = Graph::new(8, &memory)?;
        let raster = graph.add(Operation::Raster { asset, extent })?;
        assert_eq!(validate_graph(&graph, raster, asset, extent)?, raster);
        assert!(validate_graph(&graph, raster, AssetId([2; 32]), extent).is_err());
        let sparse = graph.add(Operation::Resample {
            input: raster,
            extent: Extent::new(3, 2)?,
            sampling: sugarsubstitute_image_ops::Sampling::new([3.0, 3.0], [0.0, 0.0])?
                .with_filter(Filter::Nearest),
        })?;
        assert!(validate_graph(&graph, sparse, asset, extent).is_err());
        assert!(GpuEvaluator::raster_tile_capacity(&graph, sparse).is_err());
        let wide = graph.add(Operation::Raster { asset, extent: Extent::new(259, 133)? })?;
        let filtered = graph.add(Operation::Resample {
            input: wide,
            extent: Extent::new(130, 67)?,
            sampling: sugarsubstitute_image_ops::Sampling::new([2.0, 2.0], [0.0, 0.0])?,
        })?;
        assert_eq!(GpuEvaluator::raster_tile_capacity(&graph, filtered)?, 8);
        Ok(())
    }
}
