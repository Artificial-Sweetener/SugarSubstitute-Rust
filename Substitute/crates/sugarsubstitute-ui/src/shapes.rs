// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Build the small rounded silhouettes shared by the console and loading bar.

use tiny_skia::{Path, PathBuilder, PathSegment};

#[path = "shapes_composite.rs"]
mod composite;
#[path = "shapes_raster.rs"]
mod raster;
pub use composite::PaintCursor;
pub use raster::CoverageRaster;
use raster::cut;

/// Report a required CPU coverage buffer that could not be allocated.
#[derive(Debug)]
pub enum AllocationError {
    /// Reject a mask whose dimensions cannot form a physical raster.
    InvalidDimensions,
    /// Retain the allocator's reason for rejecting the required buffer.
    Buffer(std::collections::TryReserveError),
}

impl std::fmt::Display for AllocationError {
    /// Preserve a specific buffer-allocation failure for renderer diagnostics.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimensions => formatter.write_str("Invalid native UI coverage dimensions"),
            Self::Buffer(error) => {
                write!(formatter, "Cannot allocate native UI coverage buffer: {error}")
            }
        }
    }
}

impl std::error::Error for AllocationError {
    /// Retain the native allocation cause when a mask reservation failed.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDimensions => None,
            Self::Buffer(error) => Some(error),
        }
    }
}

/// Construct the reference silhouette in physical pixels after adaptive corner flattening.
pub fn rounded(x: f32, y: f32, width: f32, height: f32, radius: f32, scale: f32) -> Option<Path> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let (x, y, width, height, radius, scale) = (
        f64::from(x),
        f64::from(y),
        f64::from(width),
        f64::from(height),
        f64::from(radius),
        f64::from(scale),
    );
    let horizontal = radius.min(width / 2.0);
    let vertical = radius.min(height / 2.0);
    let horizontal_curve = horizontal * 0.552_284_749_8;
    let vertical_curve = vertical * 0.552_284_749_8;
    let right = x + width;
    let bottom = y + height;
    let mut path = PathBuilder::new();
    path.move_to(snap(x + horizontal, scale), snap(y, scale));
    path.line_to(snap(right - horizontal, scale), snap(y, scale));
    corner(
        &mut path,
        [
            [right - horizontal, y],
            [right - horizontal + horizontal_curve, y],
            [right, y + vertical - vertical_curve],
            [right, y + vertical],
        ],
        scale,
        9,
    );
    path.line_to(snap(right, scale), snap(bottom - vertical, scale));
    corner(
        &mut path,
        [
            [right, bottom - vertical],
            [right, bottom - vertical + vertical_curve],
            [right - horizontal + horizontal_curve, bottom],
            [right - horizontal, bottom],
        ],
        scale,
        9,
    );
    path.line_to(snap(x + horizontal, scale), snap(bottom, scale));
    corner(
        &mut path,
        [
            [x + horizontal, bottom],
            [x + horizontal - horizontal_curve, bottom],
            [x, bottom - vertical + vertical_curve],
            [x, bottom - vertical],
        ],
        scale,
        9,
    );
    path.line_to(snap(x, scale), snap(y + vertical, scale));
    corner(
        &mut path,
        [
            [x, y + vertical],
            [x, y + vertical - vertical_curve],
            [x + horizontal - horizontal_curve, y],
            [x + horizontal, y],
        ],
        scale,
        9,
    );
    path.close();
    path.finish()
}

/// Snap flattened vertices to the reference painter's 26.6 device-coordinate grid.
fn snap(value: f64, scale: f64) -> f32 {
    ((value * scale * 64.0).round() / 64.0) as f32
}

/// Subdivide corners by their chord error before the device transform and grid snapping.
fn corner(path: &mut PathBuilder, points: [[f64; 2]; 4], scale: f64, depth: u8) {
    let [a, b, c, d] = points;
    let chord = [d[0] - a[0], d[1] - a[1]];
    let length = chord[0].abs() + chord[1].abs();
    let (distance, length) = if length > 1.0 {
        let cross =
            |point: [f64; 2]| (chord[0] * (a[1] - point[1]) - chord[1] * (a[0] - point[0])).abs();
        (cross(b) + cross(c), length)
    } else {
        ((a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[0] - c[0]).abs() + (a[1] - c[1]).abs(), 1.0)
    };
    if depth == 0 || distance < 0.25 / scale * length {
        path.line_to(snap(d[0], scale), snap(d[1], scale));
        return;
    }
    let midpoint =
        |left: [f64; 2], right: [f64; 2]| [(left[0] + right[0]) * 0.5, (left[1] + right[1]) * 0.5];
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    let cd = midpoint(c, d);
    let abc = midpoint(ab, bc);
    let bcd = midpoint(bc, cd);
    let middle = midpoint(abc, bcd);
    corner(path, [a, ab, abc, middle], scale, depth - 1);
    corner(path, [middle, bcd, cd, d], scale, depth - 1);
}

/// Integrate a convex physical silhouette's pixel area without coarse sample coverage.
///
/// Return the required buffer's allocation cause or invalid raster dimensions.
#[cfg(test)]
pub(crate) fn coverage(
    path: &Path,
    width: u32,
    height: u32,
) -> Result<crate::bitmap::CoverageMask, AllocationError> {
    let mut raster = CoverageRaster::new(path, width, height, true)?;
    while !raster.step(8192) {}
    raster.finish()
}

/// Append an adaptively flattened logical cubic with snapped physical vertices.
pub fn append_cubic(path: &mut PathBuilder, points: [[f64; 2]; 4], scale: f64) {
    corner(path, points, scale, 9);
}

/// Intersect a physical silhouette with its flat completion and console-facing cuts.
pub fn intersect(path: &Path, right: f32, bottom: f32) -> Option<Path> {
    let polygon: Vec<[f64; 2]> = path
        .segments()
        .filter_map(|segment| match segment {
            PathSegment::MoveTo(point) | PathSegment::LineTo(point) => {
                Some([f64::from(point.x), f64::from(point.y)])
            }
            _ => None,
        })
        .collect();
    let clipped = cut(&cut(&polygon, 0, f64::from(right), false), 1, f64::from(bottom), false);
    let first = clipped.first()?;
    let mut result = PathBuilder::new();
    result.move_to(first[0] as f32, first[1] as f32);
    for point in &clipped[1..] {
        result.line_to(point[0] as f32, point[1] as f32);
    }
    result.close();
    result.finish()
}

/// Round a byte channel's normalized coverage before source-over composition.
pub fn byte_product(channel: u8, coverage: u8) -> u8 {
    let product = u16::from(channel) * u16::from(coverage);
    ((product + (product >> 8) + 128) >> 8) as u8
}

/// Blend one straight-alpha color with coverage into a premultiplied destination.
/// Coordinates outside the target are clipped without changing its storage.
pub fn blend_pixel(
    target: &mut crate::bitmap::Bitmap,
    x: i64,
    y: i64,
    color: [u8; 4],
    coverage: u8,
) {
    if x < 0 || y < 0 || x >= i64::from(target.width()) || y >= i64::from(target.height()) {
        return;
    }
    let index = (y as usize * target.width() as usize + x as usize) * 4;
    let alpha = byte_product(color[3], coverage);
    let source = [
        byte_product(byte_product(color[0], color[3]), coverage),
        byte_product(byte_product(color[1], color[3]), coverage),
        byte_product(byte_product(color[2], color[3]), coverage),
        alpha,
    ];
    for (destination, source) in target.data_mut()[index..index + 4].iter_mut().zip(source) {
        *destination = source + byte_product(*destination, 255 - alpha);
    }
}

#[cfg(test)]
#[path = "shapes_tests.rs"]
mod tests;
