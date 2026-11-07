// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project one overlay geometry stream for pixel drawing and conservative capacity admission.
use super::*;

/// One authoritative geometry traversal serves both allocation admission and actual drawing.
pub(super) fn trace(
    overlay: &PresentationOverlay,
    view: Viewport,
    raster: &mut Raster<'_>,
) -> Result<(), RenderError> {
    if let Some(boundary) = &overlay.selection {
        let [dx, dy] = overlay.selection_offset.map(|value| value as f64);
        let stroke = Stroke::Ants(overlay.ants);
        for only in [ANT_LIGHT, ANT_DARK] {
            for run in boundary.runs() {
                let point =
                    |[x, y]: [u32; 2]| view.device_at([f64::from(x) + dx, f64::from(y) + dy]);
                let (from, to) = (point(run.start)?, point(run.end())?);
                raster.segment(from, to, 0.0, stroke, None, Some(only))?;
            }
            raster.control()?;
        }
    }
    if let Some(outline) = &overlay.outline {
        let document = overlay.document;
        let [left, top] = view.device_at([0.0, 0.0])?;
        let [right, bottom] =
            view.device_at([f64::from(document.width()), f64::from(document.height())])?;
        let clip = Some([left.floor(), top.floor(), right.ceil(), bottom.ceil()]);
        let points = outline_points(outline, view)?;
        match outline {
            OverlayOutline::Line(_) => {
                raster.segment(points[0], points[1], 0.0, Stroke::Outline, clip, None)?;
            }
            _ => raster.closed(&points, Stroke::Outline, clip)?,
        }
    }
    if let Some(guides) = &overlay.guides {
        raster.guides(guides, view)?;
    }
    for guide in overlay.snaps.iter() {
        let (start, end) = (view.device_at(guide.start)?, view.device_at(guide.end)?);
        raster.segment(start, end, 0.0, Stroke::Solid(SNAP_GUIDE), None, None)?;
    }
    raster.control()
}

/// A shape's outline in device coordinates: a closed polyline, or a line's two end points.
/// Ellipses are flattened in device space so they stay smooth at every zoom.
fn outline_points(outline: &OverlayOutline, view: Viewport) -> Result<Vec<[f64; 2]>, RenderError> {
    Ok(match outline {
        OverlayOutline::Rectangle([left, top, right, bottom]) => vec![
            view.device_at([*left, *top])?,
            view.device_at([*right, *top])?,
            view.device_at([*right, *bottom])?,
            view.device_at([*left, *bottom])?,
        ],
        OverlayOutline::Ellipse([left, top, right, bottom]) => {
            let [x0, y0] = view.device_at([*left, *top])?;
            let [x1, y1] = view.device_at([*right, *bottom])?;
            let centre = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
            let radii = [(x1 - x0) / 2.0, (y1 - y0) / 2.0];
            let perimeter = std::f64::consts::TAU * radii[0].max(radii[1]);
            let segments = ((perimeter / 2.0).ceil() as usize).clamp(16, 8192);
            (0..segments)
                .map(|index| {
                    let angle = std::f64::consts::TAU * index as f64 / segments as f64;
                    [centre[0] + radii[0] * angle.cos(), centre[1] + radii[1] * angle.sin()]
                })
                .collect()
        }
        OverlayOutline::Polygon(points) => {
            points.iter().map(|point| view.device_at(*point)).collect::<Result<_, _>>()?
        }
        OverlayOutline::Line([from, to]) => vec![view.device_at(*from)?, view.device_at(*to)?],
    })
}
