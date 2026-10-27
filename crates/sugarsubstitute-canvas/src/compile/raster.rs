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

/// Select the nearest permitted ready pyramid and preserve the exact requested output mapping.
pub(super) fn raster_node(
    graph: &mut Graph,
    asset: AssetId,
    grid: RasterGrid<'_>,
    clip: bool,
    sampling_scale: &mut f64,
) -> Result<NodeId, CanvasError> {
    let RasterGrid { document, view, reconstruction, ready } = grid;
    let source = graph.add(Operation::Raster { asset, extent: document })?;
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
        if reconstruction.filter() == Filter::Bilinear
            && let Some(ready) = ready
        {
            let required = selected;
            for &level in pyramid.levels().iter().skip_while(|&&level| level != required) {
                let extent = graph.extent(level)?;
                let footprint = raster_sampling(document, extent, view, reconstruction)?
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
        .min(f64::from(extent.width()) / f64::from(document.width()))
        .min(f64::from(extent.height()) / f64::from(document.height()));
    let sampling = raster_sampling(document, extent, view, reconstruction)?;
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

fn raster_sampling(
    document: Extent,
    extent: Extent,
    view: Viewport,
    reconstruction: Reconstruction,
) -> Result<Sampling, ImageError> {
    let ratios = [
        f64::from(extent.width()) / f64::from(document.width()),
        f64::from(extent.height()) / f64::from(document.height()),
    ];
    Ok(Sampling::new(
        [ratios[0] / view.zoom(), ratios[1] / view.zoom()],
        [view.untranslated().origin()[0] * ratios[0], view.untranslated().origin()[1] * ratios[1]],
    )?
    .with_filter(reconstruction.filter())
    .with_output_translation(view.pixel_translation()))
}
