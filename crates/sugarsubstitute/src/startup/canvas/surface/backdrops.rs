// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map page material geometry onto the independently verified native viewport pixel grid.
use sugarsubstitute_canvas_render::backdrop::BackdropRegion;
use sugarsubstitute_contracts::canvas_surface::CanvasSurfaceGeometry;
use sugarsubstitute_desktop::canvas::placement::{
    PlacementError, WebViewAllocation, preparation_viewport,
};

/// Preserve fractional bar edges after native viewport origin rounding and density validation.
/// Hidden geometry withdraws effects. Rendering independently clips padded crops and masks.
/// # Errors
/// Rejects stale native density and unrepresentable physical material geometry.
pub(super) fn project(
    geometry: &CanvasSurfaceGeometry,
    allocation: WebViewAllocation,
) -> Result<Vec<BackdropRegion>, PlacementError> {
    if !geometry.visible() || geometry.backdrops().len() == 0 {
        return Ok(Vec::new());
    }
    let viewport = preparation_viewport(geometry, allocation)?;
    let scale = allocation.physical_pixels_per_css_pixel;
    let (x, y) = geometry.origin();
    let offset_x = f64::from(allocation.bounds.x) + x * scale - f64::from(viewport.x);
    let offset_y = f64::from(allocation.bounds.y) + y * scale - f64::from(viewport.y);
    geometry
        .backdrops()
        .map(|region| {
            let [x, y, width, height] = region.bounds();
            // Validated CSS sigma and native density bound the finite physical value to 4096.
            #[allow(clippy::cast_possible_truncation)]
            let sigma = (region.sigma() * scale) as f32;
            BackdropRegion::new(
                [offset_x + x * scale, offset_y + y * scale, width * scale, height * scale],
                region.corner_radius() * scale,
                sigma,
            )
            .map_err(|_| PlacementError::InvalidAllocation)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::error::Error;
    use sugarsubstitute_desktop::canvas::placement::PhysicalRect;

    /// Keep fractional page coordinates independent of native integer viewport bounds.
    fn geometry(visible: bool) -> Result<CanvasSurfaceGeometry, Box<dyn Error>> {
        Ok(CanvasSurfaceGeometry::decode(&serde_json::to_vec(&json!({
            "coordinateSpace": "client", "units": "css-pixels",
            "origin": {"x": 2.25, "y": -1.5},
            "viewport": {"width": 400, "height": 300},
            "clip": {"x": 0, "y": 0, "width": 400, "height": 300},
            "physicalPixelsPerCssPixel": 1.5, "visible": visible, "focused": false,
            "backdrops": [{"x": 8, "y": 250, "width": 200, "height": 40,
                "cornerRadius": 8, "sigma": 10}]
        }))?)?)
    }

    #[test]
    fn material_tracks_native_grid_without_rounding_away_fractional_edges()
    -> Result<(), Box<dyn Error>> {
        let allocation = WebViewAllocation {
            bounds: PhysicalRect { x: 20, y: 30, width: 1000, height: 900 },
            physical_pixels_per_css_pixel: 1.5,
        };
        let projected = project(&geometry(true)?, allocation)?;
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].bounds(), [12.375, 375.75, 300.0, 60.0]);
        assert_eq!(projected[0].corner_radius(), 12.0);
        assert_eq!(projected[0].sigma(), 15.0);
        assert!(project(&geometry(false)?, allocation)?.is_empty());
        let stale = WebViewAllocation { physical_pixels_per_css_pixel: 2.0, ..allocation };
        assert_eq!(project(&geometry(true)?, stale), Err(PlacementError::ScaleMismatch));
        Ok(())
    }
}
