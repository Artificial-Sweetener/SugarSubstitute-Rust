// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project a reported web viewport into a trusted native client allocation.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::canvas_surface::CanvasSurfaceGeometry;

/// One physical-pixel rectangle in the main window's client coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalRect {
    /// Signed horizontal position in physical client pixels.
    pub x: i32,
    /// Signed vertical position in physical client pixels.
    pub y: i32,
    /// Nonnegative physical width.
    pub width: u32,
    /// Nonnegative physical height.
    pub height: u32,
}

/// Native observations for the exact WebView receiving this geometry update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WebViewAllocation {
    /// WebView allocation in its parent window's physical client coordinates.
    pub bounds: PhysicalRect,
    /// Native-observed physical pixels per CSS pixel, including the current page zoom.
    pub physical_pixels_per_css_pixel: f64,
}

/// A viewport and its conservative visible clip in physical client coordinates.
/// This does not prove that controls or a modal do not cover the clip, or authorize input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanvasPlacement {
    /// The complete viewport, including any portion clipped by the WebView.
    pub viewport: PhysicalRect,
    /// The inward-rounded intersection of viewport, reported clip and native allocation.
    pub clip: PhysicalRect,
}

/// Reject a stale or unrepresentable projection without moving a native surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementError {
    /// The page's reported scale does not match the current native observation.
    ScaleMismatch,
    /// An allocation, scale or converted coordinate cannot be represented safely.
    InvalidAllocation,
    /// The complete viewport exceeds the native canvas presentation limit.
    ViewportTooLarge,
}

impl fmt::Display for PlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ScaleMismatch => "Canvas scale does not match the native WebView",
            Self::InvalidAllocation => "Canvas allocation is invalid",
            Self::ViewportTooLarge => "Canvas viewport exceeds the native presentation limit",
        })
    }
}

impl Error for PlacementError {}

/// Project one current geometry snapshot without granting attachment or input authority.
///
/// The allocation must come from the current native WebView and include its actual zoom.
/// `None` means the reported viewport is hidden or has no fully covered physical pixels.
/// Clip edges round inward so fractional layout cannot expose native pixels outside the
/// reported rectangle. A separate web-occlusion owner must decide input routing.
///
/// # Errors
/// Rejects stale density, invalid native observations, overflow and unsupported extents.
pub fn placement(
    geometry: &CanvasSurfaceGeometry,
    allocation: WebViewAllocation,
) -> Result<Option<CanvasPlacement>, PlacementError> {
    if !geometry.visible() {
        return Ok(None);
    }
    let viewport = preparation_viewport(geometry, allocation)?;
    let scale = allocation.physical_pixels_per_css_pixel;
    let bounds = allocation.bounds;
    let (x, y) = geometry.origin();
    let (width, height) = geometry.viewport();
    let (clip_x, clip_y, clip_width, clip_height) = geometry.clip();
    let left = x.max(clip_x).max(0.0);
    let top = y.max(clip_y).max(0.0);
    let right = (x + width).min(clip_x + clip_width).min(f64::from(bounds.width) / scale);
    let bottom = (y + height).min(clip_y + clip_height).min(f64::from(bounds.height) / scale);
    if right <= left || bottom <= top {
        return Ok(None);
    }
    let clip = rect([left, top, right, bottom], scale, bounds, true)?;
    if clip.width == 0 || clip.height == 0 {
        return Ok(None);
    }
    Ok(Some(CanvasPlacement { viewport, clip }))
}

/// Measure a bounded physical viewport for hidden candidate preparation.
/// This validates native density and extent but grants no visibility or input authority.
/// The caller must separately obtain `placement` before presenting any native pixels.
///
/// # Errors
/// Rejects stale density, invalid native observations, overflow and unsupported extents.
pub fn preparation_viewport(
    geometry: &CanvasSurfaceGeometry,
    allocation: WebViewAllocation,
) -> Result<PhysicalRect, PlacementError> {
    let scale = allocation.physical_pixels_per_css_pixel;
    if !scale.is_finite() || !(0.0..=64.0).contains(&scale) || scale == 0.0 {
        return Err(PlacementError::InvalidAllocation);
    }
    let ratio = geometry.density() / scale;
    if !ratio.is_finite() || (ratio - 1.0).abs() > 0.000_001 {
        return Err(PlacementError::ScaleMismatch);
    }
    let bounds = allocation.bounds;
    if i64::from(bounds.x) + i64::from(bounds.width) > i64::from(i32::MAX)
        || i64::from(bounds.y) + i64::from(bounds.height) > i64::from(i32::MAX)
    {
        return Err(PlacementError::InvalidAllocation);
    }
    let (x, y) = geometry.origin();
    let (width, height) = geometry.viewport();
    let viewport = rect([x, y, x + width, y + height], scale, bounds, false)?;
    if viewport.width > super::MAX_VIEWPORT_EDGE || viewport.height > super::MAX_VIEWPORT_EDGE {
        return Err(PlacementError::ViewportTooLarge);
    }
    Ok(viewport)
}

/// Round outer viewport edges out and clip edges in before adding the native allocation origin.
fn rect(
    edges: [f64; 4],
    scale: f64,
    bounds: PhysicalRect,
    inward: bool,
) -> Result<PhysicalRect, PlacementError> {
    let [left, top, right, bottom] = edges;
    let start = |coordinate: f64, origin: i32| {
        edge(coordinate * scale, origin, if inward { f64::ceil } else { f64::floor })
    };
    let end = |coordinate: f64, origin: i32| {
        edge(coordinate * scale, origin, if inward { f64::floor } else { f64::ceil })
    };
    let x = start(left, bounds.x)?;
    let y = start(top, bounds.y)?;
    let right = end(right, bounds.x)?;
    let bottom = end(bottom, bounds.y)?;
    Ok(PhysicalRect {
        x,
        y,
        width: u32::try_from((i64::from(right) - i64::from(x)).max(0))
            .map_err(|_| PlacementError::InvalidAllocation)?,
        height: u32::try_from((i64::from(bottom) - i64::from(y)).max(0))
            .map_err(|_| PlacementError::InvalidAllocation)?,
    })
}

/// Reject overflow before adding the trusted client origin.
fn edge(value: f64, origin: i32, round: fn(f64) -> f64) -> Result<i32, PlacementError> {
    let value = round(value);
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(PlacementError::InvalidAllocation);
    }
    // The selected rounding operation produced an integer within the checked i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let offset = value as i32;
    origin.checked_add(offset).ok_or(PlacementError::InvalidAllocation)
}

#[cfg(test)]
#[path = "placement/tests.rs"]
mod tests;
