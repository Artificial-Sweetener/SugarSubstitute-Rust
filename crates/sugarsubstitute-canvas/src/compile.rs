// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compile authored content into explicit image operations at the requested evaluation grid.
use crate::{CanvasError, DocumentSnapshot, LayerContent, LayerMask, Viewport};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_image_ops::{
    AssetId, Extent, Filter, Graph, NodeId, Operation, Pixel, Sampling, VectorRectangle,
};

/// One immutable image graph for a document revision and physical evaluation grid.
/// Source pyramids use canonical content keys; vectors rasterize at the requested device resolution.
pub struct CompiledView {
    graph: Graph,
    output: NodeId,
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
    /// Transfer immutable semantics to a CPU or GPU evaluation adapter.
    pub fn into_parts(self) -> (Graph, NodeId) {
        (self.graph, self.output)
    }
}

impl DocumentSnapshot {
    /// Compile this revision at the viewport's evaluation grid without changing authored content.
    /// Raster layers use Lanczos pyramids or sparse nearest previews; vector coverage uses
    /// transformed geometry at this resolution. Adjustments and composition operate on that same
    /// linear-light grid. At actual size, one document pixel corresponds to one device pixel.
    /// # Errors
    /// Reports invalid matte/coordinates, graph limits and original memory/operation failures.
    pub fn compile_view(
        &self,
        view: Viewport,
        matte: Pixel,
        filter: Filter,
        memory: &MemoryLease,
    ) -> Result<CompiledView, CanvasError> {
        if matte.channels()[3] != 1.0 {
            return Err(CanvasError::Invalid("Canvas matte must be opaque"));
        }
        let target = view.surface();
        let mut graph = Graph::new(graph_capacity(self), memory)?;
        let background = graph.add(Operation::Solid { extent: target, color: matte })?;
        let document = self.extent();
        if !view.intersects_document(document) {
            return Ok(CompiledView { graph, output: background });
        }
        let mut composite =
            graph.add(Operation::Solid { extent: target, color: Pixel::TRANSPARENT })?;
        for layer in self.layers().iter().filter(|layer| layer.visible() && layer.opacity() > 0.0) {
            match layer.content() {
                LayerContent::Exposure { stops } => {
                    let adjusted = graph.add(Operation::Exposure { input: composite, stops })?;
                    let mask = mask_node(&mut graph, layer.mask(), document, view, filter)?;
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
                            raster_node(&mut graph, asset, document, view, filter, true)?
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
                        let mask = mask_node(&mut graph, layer.mask(), document, view, filter)?;
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
        Ok(CompiledView { graph, output })
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
    document: Extent,
    view: Viewport,
    filter: Filter,
) -> Result<NodeId, CanvasError> {
    match mask {
        Some(LayerMask::Raster { asset, .. }) => {
            raster_node(graph, asset, document, view, filter, false)
        }
        Some(LayerMask::VectorRectangle(shape)) => vector_node(graph, shape, document, view),
        None => Ok(graph.add(Operation::Solid {
            extent: view.surface(),
            color: Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
        })?),
    }
}

fn raster_node(
    graph: &mut Graph,
    asset: AssetId,
    document: Extent,
    view: Viewport,
    filter: Filter,
    clip: bool,
) -> Result<NodeId, CanvasError> {
    let source = graph.add(Operation::Raster { asset, extent: document })?;
    let mut selected = source;
    if filter == Filter::Lanczos3 {
        let pyramid = graph.pyramid(source)?;
        for &level in pyramid.levels() {
            let extent = graph.extent(level)?;
            if extent.width() == 1 && extent.height() == 1 {
                selected = level;
                break;
            }
            let scales = [
                f64::from(extent.width()) / (f64::from(document.width()) * view.zoom()),
                f64::from(extent.height()) / (f64::from(document.height()) * view.zoom()),
            ];
            let mut active = 0.0_f64;
            if extent.width() > 1 {
                active = active.max(scales[0]);
            }
            if extent.height() > 1 {
                active = active.max(scales[1]);
            }
            if active < 1.0 {
                break;
            }
            selected = level;
        }
    }
    let extent = graph.extent(selected)?;
    let ratios = [
        f64::from(extent.width()) / f64::from(document.width()),
        f64::from(extent.height()) / f64::from(document.height()),
    ];
    let sampling = Sampling::new(
        [ratios[0] / view.zoom(), ratios[1] / view.zoom()],
        [view.untranslated().origin()[0] * ratios[0], view.untranslated().origin()[1] * ratios[1]],
    )?
    .with_filter(filter)
    .with_output_translation(view.pixel_translation());
    let sampled =
        graph.add(Operation::Resample { input: selected, extent: view.surface(), sampling })?;
    if !clip {
        return Ok(sampled);
    }
    let shape = VectorRectangle::new(
        [0.0, 0.0, f64::from(document.width()), f64::from(document.height())],
        Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
    )?;
    let mask = vector_node(graph, shape, document, view)?;
    Ok(graph.add(Operation::Mask { input: sampled, mask })?)
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
