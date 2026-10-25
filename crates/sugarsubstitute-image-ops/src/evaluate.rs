// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded demand-driven CPU evaluation with feature-owned process-local tile reuse.
use crate::{
    AssetId, Graph, ImageError, NodeId, Operation, Pixel, Region, Sampling, TILE_EDGE, Tile,
    buffer::Buffer,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};

/// Read-only asset boundary. Implementations provide exact canonical region pixels in row-major order.
/// Reads execute on the caller's admitted worker, observe cancellation/deadlines, and bound their
/// own I/O/decoder workspace. They must never decode an unadmitted full image or spawn workers.
pub trait RasterSource {
    /// Fill exactly `region.pixels()` already-admitted output pixels for immutable asset content.
    /// # Errors
    /// Reports unavailable/corrupt sources and preserves original I/O/decoder causes.
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError>;
}

/// Cumulative observable work, useful for tests and headless performance qualification.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EvaluationStats {
    /// Tiles actually computed, including intermediate graph nodes.
    pub computed_tiles: u64,
    /// Pixels actually computed across all operation regions, including intermediate work.
    pub computed_pixels: u64,
    /// Requests served by an existing immutable tile allocation.
    pub reused_tiles: u64,
    /// Exact source pixels requested through the asset boundary.
    pub source_pixels: u64,
    /// Number of graph tiles retained by the evaluator, excluding outside owners.
    pub resident_tiles: usize,
}

struct CachedTile {
    key: [u8; 32],
    region: Region,
    used: u64,
    tile: Arc<Tile>,
}

/// CPU evaluator with bounded process-local retention and execution-backed allocations.
/// This is image reuse policy, not a scheduler, memory admission owner or persistent cache.
/// Move it between owned jobs; native presentation receives immutable tiles only.
pub struct Evaluator<S> {
    graph: Graph,
    source: S,
    cache: Vec<CachedTile>,
    limit: usize,
    tick: u64,
    stats: EvaluationStats,
    memory: MemoryLease,
    _cache_memory: MemoryClaim,
}
impl<S: RasterSource> Evaluator<S> {
    /// Retain a graph/source and bounded tile inventory under the original execution reservation.
    /// # Errors
    /// Rejects excessive/zero tile retention, allocator failure and insufficient admitted memory.
    pub fn new(
        graph: Graph,
        source: S,
        maximum_tiles: usize,
        memory: &MemoryLease,
    ) -> Result<Self, ImageError> {
        if maximum_tiles == 0 || maximum_tiles > 4096 {
            return Err(ImageError::Invalid("Invalid tile retention limit"));
        }
        let claim = memory.claim((maximum_tiles * size_of::<CachedTile>()) as u64)?;
        let mut cache = Vec::new();
        cache.try_reserve_exact(maximum_tiles)?;
        Ok(Self {
            graph,
            source,
            cache,
            limit: maximum_tiles,
            tick: 0,
            stats: EvaluationStats::default(),
            memory: memory.clone(),
            _cache_memory: claim,
        })
    }
    /// Immutable semantics used for all evaluation requests.
    pub const fn graph(&self) -> &Graph {
        &self.graph
    }
    /// Replace graph semantics while retaining tiles with identical structural content keys.
    /// Source adapters must keep every asset ID bound to the same immutable original content.
    /// Call on the preparation worker so retiring the preceding graph does not burden native input.
    pub fn replace_graph(&mut self, graph: Graph) {
        self.graph = graph;
    }
    /// Current cumulative work and process-local retention.
    pub fn stats(&self) -> EvaluationStats {
        EvaluationStats { resident_tiles: self.cache.len(), ..self.stats }
    }
    /// Release the evaluator's tile references. Outside retained results keep their charges.
    pub fn clear_tiles(&mut self) {
        self.cache.clear();
    }
    /// Evaluate one image-space tile. Edge tiles clip to the node's extent.
    /// Returned pixels remain charged through their final owner, after this job or evaluator ends.
    /// # Errors
    /// Reports invalid requests, memory/allocator pressure, cancellation/deadline and source errors.
    pub fn tile(
        &mut self,
        node: NodeId,
        x: u32,
        y: u32,
        context: &JobContext,
    ) -> Result<Arc<Tile>, ImageError> {
        let region = crate::spatial::tile_region(self.graph.extent(node)?, x, y)?;
        self.region(node, region, context)
    }
    /// Evaluate an exact bounded output region, preserving global sample phase at repair edges.
    /// Dependencies still reuse aligned source/pyramid tiles; a narrow exposed strip does not
    /// force reconstruction of an entire output tile. Width and height are at most `TILE_EDGE`.
    /// # Errors
    /// Rejects out-of-bounds/excessive regions and preserves admission, control and source errors.
    pub fn region(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<Arc<Tile>, ImageError> {
        check(context)?;
        if region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || region.intersection(self.graph.extent(node)?.region()) != Some(region)
        {
            return Err(ImageError::Invalid("Invalid bounded evaluation region"));
        }
        let key = self.graph.fingerprint(node)?;
        self.tick = self.tick.saturating_add(1);
        if let Some(entry) = self.cache.iter_mut().find(|v| v.key == key && v.region == region) {
            entry.used = self.tick;
            self.stats.reused_tiles = self.stats.reused_tiles.saturating_add(1);
            return Ok(Arc::clone(&entry.tile));
        }
        let operation = self.graph.operation(node)?;
        let mut output = self.allocate(region)?;
        match operation {
            Operation::Raster { asset, .. } => {
                self.source.read(asset, region, &mut output.pixels, context)?;
                for pixel in &output.pixels {
                    Pixel::premultiplied(pixel.channels())?;
                }
                self.stats.source_pixels = self.stats.source_pixels.saturating_add(region.pixels());
            }
            Operation::Solid { color, .. } => output.pixels.fill(color),
            Operation::Rectangle { shape, .. } => {
                for y in region.y()..region.bottom() {
                    check(context)?;
                    let vertical = ((f64::from(y + 1).min(shape.bounds[3])
                        - f64::from(y).max(shape.bounds[1]))
                    .clamp(0.0, 1.0)) as f32;
                    for x in region.x()..region.right() {
                        let horizontal = ((f64::from(x + 1).min(shape.bounds[2])
                            - f64::from(x).max(shape.bounds[0]))
                        .clamp(0.0, 1.0)) as f32;
                        let coverage = horizontal * vertical;
                        let index = (y - region.y()) as usize * region.width() as usize
                            + (x - region.x()) as usize;
                        output.pixels[index] = Pixel(shape.color.0.map(|value| value * coverage));
                    }
                }
            }
            Operation::Resample { input, sampling, .. } => {
                let dependency = self.graph.required_inputs(node, region)?[0]
                    .ok_or(ImageError::Invalid("Missing resample dependency"))?
                    .1;
                let input_pixels = self.input(input, dependency, context)?;
                let mut scratch = self.allocate(Region::new(
                    region.x(),
                    dependency.y(),
                    region.width(),
                    dependency.height(),
                )?)?;
                let _tables = self.claim(32 * 1024)?;
                crate::resample::render(
                    &input_pixels,
                    self.graph.extent(input)?,
                    sampling,
                    &mut output,
                    &mut scratch,
                    context,
                )?;
            }
            Operation::Exposure { input, stops } => {
                let input = self.point_input(input, region, context)?;
                let factor = stops.exp2();
                for (output, input) in output.pixels.iter_mut().zip(input.pixels()) {
                    *output = Pixel::premultiplied([
                        input.0[0] * factor,
                        input.0[1] * factor,
                        input.0[2] * factor,
                        input.0[3],
                    ])?;
                }
            }
            Operation::Over { background, foreground } => {
                let background = self.point_input(background, region, context)?;
                let foreground = self.point_input(foreground, region, context)?;
                for ((output, background), foreground) in
                    output.pixels.iter_mut().zip(background.pixels()).zip(foreground.pixels())
                {
                    *output = Pixel::premultiplied(foreground.over(*background).channels())?;
                }
            }
            Operation::Mask { input, mask } => {
                let mask = self.point_input(mask, region, context)?;
                if mask.pixels().iter().any(|pixel| pixel.0[3] > 0.0) {
                    let input = self.point_input(input, region, context)?;
                    for ((output, input), mask) in
                        output.pixels.iter_mut().zip(input.pixels()).zip(mask.pixels())
                    {
                        *output = Pixel(input.0.map(|value| value * mask.0[3]));
                    }
                }
            }
            Operation::Mix { original, adjusted, mask, opacity } => {
                let original = self.point_input(original, region, context)?;
                let mask = self.point_input(mask, region, context)?;
                if opacity == 0.0 || mask.pixels().iter().all(|pixel| pixel.0[3] == 0.0) {
                    output.pixels.copy_from_slice(original.pixels());
                } else {
                    let adjusted = self.point_input(adjusted, region, context)?;
                    for (((output, original), adjusted), mask) in output
                        .pixels
                        .iter_mut()
                        .zip(original.pixels())
                        .zip(adjusted.pixels())
                        .zip(mask.pixels())
                    {
                        let coverage = opacity * mask.0[3];
                        *output = Pixel::premultiplied(std::array::from_fn(|channel| {
                            original.0[channel] * (1.0 - coverage) + adjusted.0[channel] * coverage
                        }))?;
                    }
                }
            }
        }
        check(context)?;
        let tile = Arc::new(Tile { buffer: output });
        if self.cache.len() == self.limit {
            let index = self
                .cache
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| v.used)
                .map(|(i, _)| i)
                .ok_or(ImageError::Invalid("Empty full tile cache"))?;
            self.cache.swap_remove(index);
        }
        self.cache.push(CachedTile { key, region, used: self.tick, tile: Arc::clone(&tile) });
        self.stats.computed_tiles = self.stats.computed_tiles.saturating_add(1);
        self.stats.computed_pixels = self.stats.computed_pixels.saturating_add(region.pixels());
        Ok(tile)
    }
    /// Keep intrinsic image/mip storage aligned while propagating exact regions through operations.
    fn point_input(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<Arc<Tile>, ImageError> {
        let aligned = match self.graph.operation(node)? {
            Operation::Raster { .. } => true,
            Operation::Resample { input, extent, sampling } => {
                sampling == Sampling::resize(self.graph.extent(input)?, extent)
            }
            _ => false,
        };
        if !aligned {
            return self.region(node, region, context);
        }
        let x = region.x() / TILE_EDGE;
        let y = region.y() / TILE_EDGE;
        if crate::spatial::tile_region(self.graph.extent(node)?, x, y)? == region {
            return self.tile(node, x, y, context);
        }
        Ok(Arc::new(Tile { buffer: self.input(node, region, context)? }))
    }
    /// Gather only a bounded operation halo from source graph tiles, releasing each borrowed tile
    /// before requesting the next. Recursive pyramid evaluation never gathers a full base image.
    fn input(
        &mut self,
        node: NodeId,
        region: Region,
        context: &JobContext,
    ) -> Result<Buffer, ImageError> {
        let mut buffer = self.allocate(region)?;
        for y in region.y() / TILE_EDGE..=(region.bottom() - 1) / TILE_EDGE {
            for x in region.x() / TILE_EDGE..=(region.right() - 1) / TILE_EDGE {
                let tile = self.tile(node, x, y, context)?;
                let overlap = tile
                    .region()
                    .intersection(region)
                    .ok_or(ImageError::Invalid("Missing input tile intersection"))?;
                for row in overlap.y()..overlap.bottom() {
                    let source = (row - tile.region().y()) as usize
                        * tile.region().width() as usize
                        + (overlap.x() - tile.region().x()) as usize;
                    let target = (row - region.y()) as usize * region.width() as usize
                        + (overlap.x() - region.x()) as usize;
                    let count = overlap.width() as usize;
                    buffer.pixels[target..target + count]
                        .copy_from_slice(&tile.pixels()[source..source + count]);
                }
            }
        }
        Ok(buffer)
    }
    fn allocate(&mut self, region: Region) -> Result<Buffer, ImageError> {
        if region.width() > 2 * TILE_EDGE + 16 || region.height() > 2 * TILE_EDGE + 16 {
            return Err(ImageError::Invalid("Operation dependency exceeds bounded tile workspace"));
        }
        let memory = self.claim(region.pixels() * size_of::<Pixel>() as u64 + 128)?;
        Buffer::new(region, memory)
    }
    fn claim(&mut self, bytes: u64) -> Result<MemoryClaim, ImageError> {
        loop {
            match self.memory.claim(bytes) {
                Ok(claim) => return Ok(claim),
                Err(cause) => {
                    let oldest = self
                        .cache
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| Arc::strong_count(&v.tile) == 1)
                        .min_by_key(|(_, v)| v.used)
                        .map(|(index, _)| index);
                    let Some(index) = oldest else {
                        return Err(cause.into());
                    };
                    self.cache.swap_remove(index);
                }
            }
        }
    }
}

pub(crate) fn check(context: &JobContext) -> Result<(), ImageError> {
    if context.cancellation_requested() {
        return Err(ImageError::Cancelled);
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(ImageError::Deadline);
    }
    Ok(())
}
