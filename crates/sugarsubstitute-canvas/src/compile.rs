// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compile authored content into explicit image operations at the requested evaluation grid.
use crate::{CanvasError, DocumentSnapshot, LayerContent, LayerMask, Viewport};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_image_ops::{
    AssetId, Extent, Filter, Graph, ImageError, NodeId, Operation, Pixel, Reconstruction, Region,
    Sampling, VectorRectangle,
};

type PyramidAvailability<'a> =
    Option<&'a dyn Fn(&Graph, NodeId, Region) -> Result<bool, ImageError>>;

mod raster;
use raster::{RasterGrid, raster_node};

/// One immutable image graph for a document revision and physical evaluation grid.
/// Source pyramids use canonical content keys; vectors rasterize at the requested device resolution.
pub struct CompiledView {
    graph: Graph,
    output: NodeId,
    sampling_scale: f64,
}
impl CompiledView {
    /// Borrow the complete graph, including source pyramids, sampling, coverage and composition.
    pub const fn graph(&self) -> &Graph {
        &self.graph
    }
    /// Output at the requested physical viewport extent, composited over the selected opaque matte.
    pub const fn output(&self) -> NodeId {
        self.output
    }
    /// Conservative samples per document pixel, limited by raster/mask pyramids and device grid.
    /// Vector-only views retain the device sampling scale.
    pub const fn sampling_scale(&self) -> f64 {
        self.sampling_scale
    }
    /// Transfer immutable semantics to a CPU or GPU evaluation adapter.
    pub fn into_parts(self) -> (Graph, NodeId) {
        (self.graph, self.output)
    }
}

impl DocumentSnapshot {
    /// Compile this revision at the viewport's evaluation grid without changing authored content.
    /// Finished raster layers use canonical Lanczos pyramid reduction followed by their selected
    /// final filter. Previews sample original rasters directly with sparse nearest reconstruction.
    /// Vector coverage, adjustments and composition use the same requested linear-light grid. At
    /// actual size, one document pixel corresponds to one device pixel.
    /// # Errors
    /// Reports invalid matte/coordinates, graph limits and original memory/operation failures.
    pub fn compile_view(
        &self,
        view: Viewport,
        matte: Pixel,
        reconstruction: Reconstruction,
        memory: &MemoryLease,
    ) -> Result<CompiledView, CanvasError> {
        self.compile(view, matte, reconstruction, memory, None)
    }

    /// Compile navigation using the nearest ready canonical pyramid covering the requested view.
    /// Bilinear raster and mask sampling can use a coarser ready level; exact viewing and nearest
    /// pixel inspection retain their required level. Availability queries must perform no I/O.
    /// Missing ready coverage retains the required level for ordinary admitted DAG preparation.
    /// # Errors
    /// Preserves compilation and availability-query errors without changing the document.
    pub fn compile_navigation_view(
        &self,
        view: Viewport,
        matte: Pixel,
        reconstruction: Reconstruction,
        memory: &MemoryLease,
        ready: impl Fn(&Graph, NodeId, Region) -> Result<bool, ImageError>,
    ) -> Result<CompiledView, CanvasError> {
        self.compile(view, matte, reconstruction, memory, Some(&ready))
    }

    fn compile(
        &self,
        view: Viewport,
        matte: Pixel,
        reconstruction: Reconstruction,
        memory: &MemoryLease,
        ready: PyramidAvailability<'_>,
    ) -> Result<CompiledView, CanvasError> {
        if matte.channels()[3] != 1.0 {
            return Err(CanvasError::Invalid("Canvas matte must be opaque"));
        }
        let target = view.surface();
        let mut graph = Graph::new(graph_capacity(self), memory)?;
        let background = graph.add(Operation::Solid { extent: target, color: matte })?;
        let document = self.extent();
        let raster = RasterGrid { document, view, reconstruction, ready };
        let mut sampling_scale = view.zoom();
        if !view.intersects_document(document) {
            return Ok(CompiledView { graph, output: background, sampling_scale });
        }
        let mut composite =
            graph.add(Operation::Solid { extent: target, color: Pixel::TRANSPARENT })?;
        for layer in self.layers().iter().filter(|layer| layer.visible() && layer.opacity() > 0.0) {
            match layer.content() {
                LayerContent::Exposure { stops } => {
                    let adjusted = graph.add(Operation::Exposure { input: composite, stops })?;
                    let mask = mask_node(&mut graph, layer.mask(), raster, &mut sampling_scale)?;
                    composite = graph.add(Operation::Mix {
                        original: composite,
                        adjusted,
                        mask,
                        opacity: layer.opacity(),
                    })?;
                }
                content => {
                    let mut foreground = match content {
                        LayerContent::Raster { asset, .. } => {
                            raster_node(&mut graph, asset, raster, true, &mut sampling_scale)?
                        }
                        LayerContent::VectorRectangle(shape) => {
                            vector_node(&mut graph, shape, document, view)?
                        }
                        LayerContent::Exposure { .. } => {
                            return Err(CanvasError::Invalid(
                                "Adjustment reached ordinary layer composition",
                            ));
                        }
                    };
                    if layer.mask().is_some() {
                        let mask =
                            mask_node(&mut graph, layer.mask(), raster, &mut sampling_scale)?;
                        foreground = graph.add(Operation::Mask { input: foreground, mask })?;
                    }
                    if layer.opacity() != 1.0 {
                        let mask = graph.add(Operation::Solid {
                            extent: target,
                            color: Pixel::linear(0.0, 0.0, 0.0, layer.opacity())?,
                        })?;
                        foreground = graph.add(Operation::Mask { input: foreground, mask })?;
                    }
                    composite = graph.add(Operation::Over { background: composite, foreground })?;
                }
            }
        }
        let output = graph.add(Operation::Over { background, foreground: composite })?;
        Ok(CompiledView { graph, output, sampling_scale })
    }
}

/// Admit the actual maximum operation inventory for these layers, including every explicit mip.
fn graph_capacity(document: &DocumentSnapshot) -> usize {
    let extent = document.extent();
    let reductions =
        (u32::BITS - (extent.width().max(extent.height()) - 1).leading_zeros()) as usize;
    let sampled = reductions + 2;
    let mut capacity = 3;
    for layer in document.layers().iter().filter(|layer| layer.visible() && layer.opacity() > 0.0) {
        let mask = match layer.mask() {
            Some(LayerMask::Raster { .. }) => sampled,
            _ => 1,
        };
        match layer.content() {
            LayerContent::Exposure { .. } => capacity += 2 + mask,
            content => {
                capacity += match content {
                    LayerContent::Raster { .. } => sampled + 2,
                    _ => 1,
                };
                capacity += 1;
                if layer.mask().is_some() {
                    capacity += mask + 1;
                }
                if layer.opacity() != 1.0 {
                    capacity += 2;
                }
            }
        }
    }
    capacity
}

fn mask_node(
    graph: &mut Graph,
    mask: Option<LayerMask>,
    grid: RasterGrid<'_>,
    sampling_scale: &mut f64,
) -> Result<NodeId, CanvasError> {
    match mask {
        Some(LayerMask::Raster { asset, .. }) => {
            raster_node(graph, asset, grid, false, sampling_scale)
        }
        Some(LayerMask::VectorRectangle(shape)) => {
            vector_node(graph, shape, grid.document, grid.view)
        }
        None => Ok(graph.add(Operation::Solid {
            extent: grid.view.surface(),
            color: Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
        })?),
    }
}

fn vector_node(
    graph: &mut Graph,
    shape: VectorRectangle,
    document: Extent,
    view: Viewport,
) -> Result<NodeId, CanvasError> {
    let [left, top, right, bottom] = shape.bounds();
    let bounds = [
        left.max(0.0),
        top.max(0.0),
        right.min(f64::from(document.width())),
        bottom.min(f64::from(document.height())),
    ];
    if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        return Ok(
            graph.add(Operation::Solid { extent: view.surface(), color: Pixel::TRANSPARENT })?
        );
    }
    let phase = view.untranslated();
    let start = phase.device_at([bounds[0], bounds[1]])?;
    let end = phase.device_at([bounds[2], bounds[3]])?;
    let shape = VectorRectangle::new([start[0], start[1], end[0], end[1]], shape.color())?;
    Ok(graph.add(Operation::Rectangle {
        extent: view.surface(),
        shape,
        translation: view.pixel_translation(),
    })?)
}

#[cfg(test)]
#[path = "compile/tests.rs"]
mod tests;
