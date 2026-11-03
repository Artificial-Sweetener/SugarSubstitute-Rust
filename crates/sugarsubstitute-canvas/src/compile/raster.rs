// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compile raster/mask pyramid selection through a read-only backend availability boundary.
use super::*;

/// Raster/mask compilation on one document grid with an optional read-only cache boundary.
#[derive(Clone, Copy)]
pub(super) struct RasterGrid<'a> {
    pub(super) document: Extent,
    pub(super) view: Viewport,
    pub(super) reconstruction: Reconstruction,
    pub(super) ready: PyramidAvailability<'a>,
}

/// Where a source's own pixels land: its extent stretched into document-space `bounds`.
#[derive(Clone, Copy)]
pub(super) struct RasterPlacement {
    /// Document-space `[left, top, width, height]` with positive finite dimensions.
    pub(super) bounds: [f64; 4],
}

impl RasterPlacement {
    /// Stretch a source over the whole document rectangle.
    pub(super) fn document(document: Extent) -> Self {
        Self { bounds: [0.0, 0.0, f64::from(document.width()), f64::from(document.height())] }
    }
}

/// Document-resolution pixels a placement samples.
#[derive(Clone, Copy)]
pub(super) enum GraphSource<'a> {
    /// An image asset at its own pixel extent.
    Raster {
        /// Content-bound asset identity.
        asset: AssetId,
        /// Asset pixel dimensions.
        extent: Extent,
    },
    /// Document coverage, such as a layer mask.
    Coverage(&'a Coverage),
}

/// Select the nearest permitted ready pyramid and preserve the exact requested output mapping.
/// Per-axis ratios stretch the source's own extent into its placement rectangle.
pub(super) fn raster_node(
    graph: &mut Graph,
    source: GraphSource<'_>,
    placement: RasterPlacement,
    grid: RasterGrid<'_>,
    clip: bool,
    sampling_scale: &mut f64,
) -> Result<NodeId, CanvasError> {
    let RasterGrid { document, view, reconstruction, ready } = grid;
    let [_, _, width, height] = placement.bounds;
    let source = match source {
        GraphSource::Raster { asset, extent } => graph.add(Operation::Raster { asset, extent })?,
        GraphSource::Coverage(coverage) => graph.coverage(coverage)?,
    };
    let mut selected = source;
    if reconstruction.uses_canonical_pyramid() {
        let pyramid = graph.pyramid(source)?;
        for &level in pyramid.levels() {
            let extent = graph.extent(level)?;
            if extent.width() == 1 && extent.height() == 1 {
                selected = level;
                break;
            }
            let scales = [
                f64::from(extent.width()) / (width * view.zoom()),
                f64::from(extent.height()) / (height * view.zoom()),
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
        if reconstruction.filter() == Filter::Bilinear
            && let Some(ready) = ready
        {
            let required = selected;
            for &level in pyramid.levels().iter().skip_while(|&&level| level != required) {
                let extent = graph.extent(level)?;
                let footprint = raster_sampling(placement.bounds, extent, view, reconstruction)?
                    .required_input(extent, view.surface().region())?;
                if ready(graph, level, footprint)? {
                    selected = level;
                    break;
                }
            }
        }
    }
    let extent = graph.extent(selected)?;
    *sampling_scale = sampling_scale
        .min(f64::from(extent.width()) / width)
        .min(f64::from(extent.height()) / height);
    let sampling = raster_sampling(placement.bounds, extent, view, reconstruction)?;
    let sampled =
        graph.add(Operation::Resample { input: selected, extent: view.surface(), sampling })?;
    if !clip {
        return Ok(sampled);
    }
    let [left, top, width, height] = placement.bounds;
    let shape = VectorRectangle::new(
        [left, top, left + width, top + height],
        Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
    )?;
    let mask = vector_node(graph, shape, document, view)?;
    Ok(graph.add(Operation::Mask { input: sampled, mask })?)
}

fn raster_sampling(
    bounds: [f64; 4],
    extent: Extent,
    view: Viewport,
    reconstruction: Reconstruction,
) -> Result<Sampling, ImageError> {
    let [left, top, width, height] = bounds;
    let ratios = [f64::from(extent.width()) / width, f64::from(extent.height()) / height];
    let origin = view.untranslated().origin();
    Ok(Sampling::new(
        [ratios[0] / view.zoom(), ratios[1] / view.zoom()],
        [(origin[0] - left) * ratios[0], (origin[1] - top) * ratios[1]],
    )?
    .with_filter(reconstruction.filter())
    .with_output_translation(view.pixel_translation()))
}
