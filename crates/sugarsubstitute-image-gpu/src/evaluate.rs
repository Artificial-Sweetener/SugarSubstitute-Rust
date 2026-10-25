// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Evaluate the authoritative image DAG into bounded immutable GPU tiles.
use crate::{
    GpuDevice, GpuError, GpuTile,
    kernel::{Computed, Kernels, Parameters},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{
    EvaluationStats, Graph, NodeId, Operation, Pixel, RasterSource, Region, SamplingAxis,
    TILE_EDGE, TileRetention,
};

/// Demand-driven evaluator for one explicitly selected GPU and the same graph used by CPU viewing.
/// Scheduling, source storage and admission retain their existing owners. Native handles are private.
/// All methods run on an admitted preparation worker; returned tiles retain their original charges.
pub struct GpuEvaluator<S> {
    pub(crate) graph: Graph,
    pub(crate) source: S,
    pub(crate) device: Arc<GpuDevice>,
    pub(crate) kernels: Kernels,
    pub(crate) cache: TileRetention<Computed>,
    stats: EvaluationStats,
    depth: usize,
}
impl<S: RasterSource> GpuEvaluator<S> {
    /// Compile kernels and reserve bounded reuse metadata inside an admitted worker.
    /// # Errors
    /// Preserves original image, execution and driver causes; rejects incompatible storage limits.
    pub fn new(
        graph: Graph,
        source: S,
        maximum_tiles: usize,
        device: Arc<GpuDevice>,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        let cache = TileRetention::new(maximum_tiles, memory).map_err(image_error)?;
        let kernels = Kernels::new(device.clone(), context)?;
        Ok(Self {
            graph,
            source,
            device,
            kernels,
            cache,
            stats: EvaluationStats::default(),
            depth: 0,
        })
    }
    /// Immutable semantics shared by all backend evaluation requests.
    pub const fn graph(&self) -> &Graph {
        &self.graph
    }
    /// Replace document semantics while preserving references with identical structural content keys.
    /// Source IDs must remain bound to the same immutable original content across snapshots.
    pub fn replace_graph(&mut self, graph: Graph) {
        self.graph = graph;
    }
    /// Cumulative work and current semantic tile retention, excluding outside owners.
    pub fn stats(&self) -> EvaluationStats {
        EvaluationStats { resident_tiles: self.cache.len(), ..self.stats }
    }
    /// Release evaluator references; outside GPU results retain their native allocation charges.
    pub fn clear_tiles(&mut self) {
        self.cache.clear();
    }
    /// Evaluate one aligned 128-pixel image-space tile, clipping at image edges.
    /// # Errors
    /// Rejects invalid indices and preserves control, source, admission and native failures.
    pub fn tile(
        &mut self,
        node: NodeId,
        x: u32,
        y: u32,
        context: &JobContext,
    ) -> Result<GpuTile, GpuError> {
        let region =
            self.graph.extent(node).map_err(image_error)?.tile_region(x, y).map_err(image_error)?;
        self.region(node, region, context)
    }
    /// Evaluate an exact bounded region using global sampling phase, including repair strips.
    /// # Errors
    /// Rejects out-of-bounds/oversized regions; never substitutes CPU evaluation after GPU failure.
    pub fn region(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<GpuTile, GpuError> {
        Ok(self.computed(node, region, context)?.tile.clone())
    }
    pub(crate) fn computed(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<Arc<Computed>, GpuError> {
        if self.depth == 0 {
            self.cache.prefer_level(self.graph.requested_pyramid_level(node).map_err(image_error)?);
        }
        self.depth += 1;
        let result = self.compute(node, region, context);
        self.depth -= 1;
        result
    }
    fn compute(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<Arc<Computed>, GpuError> {
        crate::adapter::check(context)?;
        if region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || region.intersection(self.graph.extent(node).map_err(image_error)?.region())
                != Some(region)
        {
            return Err(GpuError::invalid("Invalid bounded GPU evaluation region"));
        }
        let key = self.graph.fingerprint(node).map_err(image_error)?;
        let level = self.graph.pyramid_level(node).map_err(image_error)?;
        if let Some(tile) = self.cache.find(key, region, level) {
            self.stats.reused_tiles = self.stats.reused_tiles.saturating_add(1);
            return Ok(tile);
        }
        let operation = self.graph.operation(node).map_err(image_error)?;
        let computed = match operation {
            Operation::Raster { asset, .. } => {
                let _memory = context
                    .memory()
                    .claim(region.pixels() * 16 + 128)
                    .map_err(|cause| GpuError::new("gpu_source_workspace", cause))?;
                let mut pixels = Vec::new();
                pixels
                    .try_reserve_exact(region.pixels() as usize)
                    .map_err(|cause| GpuError::new("gpu_source_allocation", cause))?;
                pixels.resize(region.pixels() as usize, Pixel::TRANSPARENT);
                self.source.read(asset, region, &mut pixels, context).map_err(image_error)?;
                let tile = self.retry(|this| this.device.upload(region, &pixels, context))?;
                self.stats.source_pixels = self.stats.source_pixels.saturating_add(region.pixels());
                Computed { tile, has_alpha: pixels.iter().any(|pixel| pixel.channels()[3] > 0.0) }
            }
            Operation::Solid { color, .. } => {
                let mut parameters = Parameters::new(0, region);
                parameters.color(color);
                self.dispatch(parameters, region, [None; 3], &[], context)?
            }
            Operation::Rectangle { shape, .. } => {
                let mut coefficients = [0; 256];
                for index in 0..region.width().max(region.height()) as usize {
                    coefficients[index * 2] = shape
                        .axis_coverage(SamplingAxis::Horizontal, region.x() + index as u32)
                        .to_bits();
                    coefficients[index * 2 + 1] = shape
                        .axis_coverage(SamplingAxis::Vertical, region.y() + index as u32)
                        .to_bits();
                }
                let mut parameters = Parameters::new(1, region);
                parameters.color(shape.color());
                self.dispatch(parameters, region, [None; 3], &coefficients, context)?
            }
            Operation::Resample { input, sampling, .. } => {
                self.resample(node, input, sampling, region, context)?
            }
            Operation::Exposure { input, stops } => {
                let input = self.input(input, region, context)?;
                let mut parameters = Parameters::new(2, region);
                parameters.scalar(stops.exp2());
                self.dispatch(parameters, region, [Some(&input), None, None], &[], context)?
            }
            Operation::Over { background, foreground } => {
                let background = self.input(background, region, context)?;
                let foreground = self.input(foreground, region, context)?;
                self.dispatch(
                    Parameters::new(3, region),
                    region,
                    [Some(&background), Some(&foreground), None],
                    &[],
                    context,
                )?
            }
            Operation::Mask { input, mask } => {
                let mask = self.computed(mask, region, context)?;
                if mask.has_alpha {
                    let input = self.input(input, region, context)?;
                    self.dispatch(
                        Parameters::new(4, region),
                        region,
                        [Some(&input), Some(&mask.tile), None],
                        &[],
                        context,
                    )?
                } else {
                    self.dispatch(Parameters::new(0, region), region, [None; 3], &[], context)?
                }
            }
            Operation::Mix { original, adjusted, mask, opacity } => {
                let original = self.computed(original, region, context)?;
                let mask = self.computed(mask, region, context)?;
                if opacity == 0.0 || !mask.has_alpha {
                    // Reusing immutable device storage preserves exact original pixels.
                    Computed { tile: original.tile.clone(), has_alpha: original.has_alpha }
                } else {
                    let adjusted = self.input(adjusted, region, context)?;
                    let mut parameters = Parameters::new(5, region);
                    parameters.scalar(opacity);
                    self.dispatch(
                        parameters,
                        region,
                        [Some(&original.tile), Some(&adjusted), Some(&mask.tile)],
                        &[],
                        context,
                    )?
                }
            }
        };
        crate::adapter::check(context)?;
        let computed = Arc::new(computed);
        self.cache.insert(key, region, level, computed.clone());
        self.stats.computed_tiles = self.stats.computed_tiles.saturating_add(1);
        self.stats.computed_pixels = self.stats.computed_pixels.saturating_add(region.pixels());
        Ok(computed)
    }
    pub(crate) fn dispatch(
        &mut self,
        parameters: Parameters,
        region: Region,
        inputs: [Option<&GpuTile>; 3],
        coefficients: &[u32],
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
        self.retry(|this| {
            this.kernels.dispatch(Parameters(parameters.0), region, inputs, coefficients, context)
        })
    }
    pub(crate) fn retry<T>(
        &mut self,
        mut operation: impl FnMut(&Self) -> Result<T, GpuError>,
    ) -> Result<T, GpuError> {
        loop {
            match operation(self) {
                Err(cause) if cause.memory_pressure() => {
                    if !self
                        .cache
                        .evict_unpinned(|computed| Arc::strong_count(&computed.tile.buffer) == 1)
                    {
                        return Err(cause);
                    }
                }
                result => return result,
            }
        }
    }
}
pub(crate) fn image_error(cause: sugarsubstitute_image_ops::ImageError) -> GpuError {
    GpuError::new("gpu_image", cause)
}
