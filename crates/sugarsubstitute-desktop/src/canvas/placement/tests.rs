// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify scale, clipping and native bounds without creating a window or renderer.

use super::*;
use serde_json::json;

fn geometry(
    origin: (f64, f64),
    viewport: (f64, f64),
    clip: (f64, f64, f64, f64),
    density: f64,
    visible: bool,
) -> Result<CanvasSurfaceGeometry, Box<dyn Error>> {
    Ok(CanvasSurfaceGeometry::decode(&serde_json::to_vec(&json!({
        "coordinateSpace": "client", "units": "css-pixels",
        "origin": {"x": origin.0, "y": origin.1},
        "viewport": {"width": viewport.0, "height": viewport.1},
        "clip": {"x": clip.0, "y": clip.1, "width": clip.2, "height": clip.3},
        "physicalPixelsPerCssPixel": density, "visible": visible, "focused": false
    }))?)?)
}

#[test]
fn fractional_viewport_and_clip_use_native_origin_and_conservative_edges()
-> Result<(), Box<dyn Error>> {
    let value = geometry((-2.25, 3.5), (20.0, 10.0), (-1.0, 4.2, 12.2, 7.5), 1.5, true)?;
    let allocation = WebViewAllocation {
        bounds: PhysicalRect { x: 9, y: 31, width: 30, height: 30 },
        physical_pixels_per_css_pixel: 1.5,
    };
    let result = placement(&value, allocation)?.ok_or("Visible canvas missing")?;
    assert_eq!(result.viewport, PhysicalRect { x: 5, y: 36, width: 31, height: 16 });
    assert_eq!(result.clip, PhysicalRect { x: 9, y: 38, width: 16, height: 10 });
    Ok(())
}

#[test]
fn hides_empty_or_offscreen_allocations_and_rejects_stale_scale() -> Result<(), Box<dyn Error>> {
    let allocation = WebViewAllocation {
        bounds: PhysicalRect { x: 0, y: 32, width: 400, height: 300 },
        physical_pixels_per_css_pixel: 1.0,
    };
    let hidden = geometry((0.0, 0.0), (10.0, 10.0), (0.0, 0.0, 10.0, 10.0), 1.0, false)?;
    assert_eq!(placement(&hidden, allocation)?, None);
    let outside = geometry((500.0, 0.0), (10.0, 10.0), (500.0, 0.0, 10.0, 10.0), 1.0, true)?;
    assert_eq!(placement(&outside, allocation)?, None);
    let fractional = geometry((0.2, 0.2), (0.4, 0.4), (0.2, 0.2, 0.4, 0.4), 1.0, true)?;
    assert_eq!(placement(&fractional, allocation)?, None);
    let mismatch = geometry((0.0, 0.0), (10.0, 10.0), (0.0, 0.0, 10.0, 10.0), 2.0, true)?;
    assert_eq!(placement(&mismatch, allocation), Err(PlacementError::ScaleMismatch));
    Ok(())
}

#[test]
fn hidden_preparation_validates_extent_and_density_without_granting_placement()
-> Result<(), Box<dyn Error>> {
    let allocation = WebViewAllocation {
        bounds: PhysicalRect { x: 0, y: 0, width: 800, height: 600 },
        physical_pixels_per_css_pixel: 1.5,
    };
    let hidden = geometry((10.25, 20.5), (80.0, 60.0), (0.0, 0.0, 0.0, 0.0), 1.5, false)?;
    let viewport = preparation_viewport(&hidden, allocation)?;
    assert_eq!((viewport.width, viewport.height), (121, 91));
    assert_eq!(placement(&hidden, allocation)?, None);
    let stale = WebViewAllocation { physical_pixels_per_css_pixel: 1.0, ..allocation };
    assert_eq!(preparation_viewport(&hidden, stale), Err(PlacementError::ScaleMismatch));
    let oversized = geometry((0.0, 0.0), (20_000.0, 60.0), (0.0, 0.0, 0.0, 0.0), 1.5, false)?;
    assert_eq!(preparation_viewport(&oversized, allocation), Err(PlacementError::ViewportTooLarge));
    Ok(())
}

#[test]
fn rejects_oversized_viewports_and_unrepresentable_native_origins() -> Result<(), Box<dyn Error>> {
    let value = geometry((0.0, 0.0), (20_000.0, 20.0), (0.0, 0.0, 100.0, 20.0), 1.0, true)?;
    let bounds = PhysicalRect { x: 0, y: 0, width: 100, height: 100 };
    assert_eq!(
        placement(&value, WebViewAllocation { bounds, physical_pixels_per_css_pixel: 1.0 }),
        Err(PlacementError::ViewportTooLarge)
    );
    let value = geometry((1.0, 0.0), (10.0, 10.0), (1.0, 0.0, 10.0, 10.0), 1.0, true)?;
    let bounds = PhysicalRect { x: i32::MAX, ..bounds };
    assert_eq!(
        placement(&value, WebViewAllocation { bounds, physical_pixels_per_css_pixel: 1.0 }),
        Err(PlacementError::InvalidAllocation)
    );
    Ok(())
}
