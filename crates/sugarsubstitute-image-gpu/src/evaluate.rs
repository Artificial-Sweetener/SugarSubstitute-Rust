// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Evaluate the authoritative image DAG into bounded immutable GPU tiles.
mod source;
mod sparse;
use crate::{
    GpuDevice, GpuError, GpuTile,
    batch::{Batch, Coverage, GpuWork},
    buffer::Encoding,
    kernel::{Computed, Kernels, Parameters},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{
    EvaluationStats, Graph, NodeId, Operation, Pixel, RasterSamples, RasterSource, Region,
    SamplingAxis, TILE_EDGE, TileRetention,
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
    pub(crate) batch: Option<Batch>,
    pub(crate) work: GpuWork,
}
impl<S: RasterSource> GpuEvaluator<S> {
    /// Inspect ready structural tile coverage without native calls, transfers or cache promotion.
    /// This metadata snapshot retains no native handle and grants no allocation or queue admission.
    pub fn has_cached_region(&self, content: [u8; 32], region: Region) -> bool {
        self.batch.is_none() && self.cache.contains_region(content, region)
    }
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
            batch: None,
            work: GpuWork::default(),
        })
    }
    /// Immutable semantics shared by all backend evaluation requests.
    pub const fn graph(&self) -> &Graph {
        &self.graph
    }
    /// Replace document semantics while preserving references with identical structural content keys.
    /// Source IDs must remain bound to the same immutable original content across snapshots.
    pub fn replace_graph(&mut self, graph: Graph) {
        self.cache.release_batch();
        self.graph = graph;
    }
    /// Cumulative work and current semantic tile retention, excluding outside owners.
    pub fn stats(&self) -> EvaluationStats {
        EvaluationStats { resident_tiles: self.cache.len(), ..self.stats }
    }
    /// Completed native submissions and transfer work, excluding cache-only requests.
    pub const fn work(&self) -> GpuWork {
        self.work
    }
    /// Release evaluator references; outside GPU results retain their native allocation charges.
    pub fn clear_tiles(&mut self) {
        self.cache.release_batch();
        self.cache.clear();
    }
    /// Remove one cache-only tile reference using the shared tile-retention policy.
    /// Prefer allocations freed immediately, then release cache aliases. Returns false when no
    /// cache-only tile value is eligible. Outside/native references keep their original allocations
    /// charged, so an eviction does not necessarily release bytes until the last alias disappears.
    pub fn evict_one(&mut self) -> bool {
        self.cache.evict_unpinned(|computed| Arc::strong_count(&computed.tile.buffer) == 1)
            || self.cache.evict_unpinned(|_| true)
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
        let result = self.computed(node, region, context);
        let computed = self.settle(result, context)?;
        Ok(computed.tile.clone())
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
            Operation::Raster { asset, .. } => self.source_tile(asset, region, None, context)?,
            Operation::Solid { color, .. } => {
                let mut parameters = Parameters::new(0, region);
                parameters.color(color);
                let mut computed = self.dispatch(parameters, region, [None; 3], &[], context)?;
                if let Some(coverage) = Coverage::bounded_maximum(color.channels()[3]) {
                    computed.coverage = coverage;
                }
                computed
            }
            Operation::Rectangle { shape, translation, .. } => {
                let mut coefficients = [0; 256];
                for index in 0..region.width().max(region.height()) as usize {
                    coefficients[index * 2] = shape
                        .axis_coverage(
                            SamplingAxis::Horizontal,
                            i64::from(region.x()) + index as i64 - i64::from(translation[0]),
                        )
                        .to_bits();
                    coefficients[index * 2 + 1] = shape
                        .axis_coverage(
                            SamplingAxis::Vertical,
                            i64::from(region.y()) + index as i64 - i64::from(translation[1]),
                        )
                        .to_bits();
                }
                let mut parameters = Parameters::new(1, region);
                parameters.color(shape.color());
                let horizontal = (0..region.width() as usize)
                    .map(|index| f32::from_bits(coefficients[index * 2]))
                    .fold(0.0, f32::max);
                let vertical = (0..region.height() as usize)
                    .map(|index| f32::from_bits(coefficients[index * 2 + 1]))
                    .fold(0.0, f32::max);
                let maximum = shape.color().channels()[3] * (horizontal * vertical);
                let mut computed =
                    self.dispatch(parameters, region, [None; 3], &coefficients, context)?;
                let coverage =
                    if shape.color().channels()[3] == 0.0 || horizontal == 0.0 || vertical == 0.0 {
                        Some(Coverage::Known(false))
                    } else if maximum > 0.0 {
                        Coverage::bounded_maximum(maximum)
                    } else {
                        None
                    };
                if let Some(coverage) = coverage {
                    computed.coverage = coverage;
                }
                computed
            }
            Operation::Resample { input, sampling, .. } => {
                if let Some((asset, samples)) =
                    self.graph.sampled_source(node, region).map_err(image_error)?
                {
                    self.source_tile(asset, region, Some(&samples), context)?
                } else {
                    self.resample(node, input, sampling, region, context)?
                }
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
                if self.coverage(&mask)? {
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
                if opacity == 0.0 || !self.coverage(&mask)? {
                    // Reusing immutable device storage preserves exact original pixels.
                    Computed { tile: original.tile.clone(), coverage: original.coverage.clone() }
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
            this.ready(context)?;
            let batch = this
                .batch
                .as_mut()
                .ok_or_else(|| GpuError::invalid("Image batch is unavailable"))?;
            this.kernels.encode(
                batch,
                Parameters(parameters.0),
                region,
                inputs,
                coefficients,
                context,
            )
        })
    }
    pub(crate) fn retry<T>(
        &mut self,
        mut operation: impl FnMut(&mut Self) -> Result<T, GpuError>,
    ) -> Result<T, GpuError> {
        let mut drained = false;
        loop {
            match operation(self) {
                Err(cause) if cause.is_memory_pressure() => {
                    let had_batch = self.batch.is_some();
                    self.finish_batch()?;
                    if had_batch && !drained {
                        drained = true;
                        continue;
                    }
                    if !self.evict_one() {
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
