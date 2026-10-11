// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Build the small rounded silhouettes shared by the console and loading bar.

use tiny_skia::{Mask, Path, PathBuilder, PathSegment};

/// Report a required CPU coverage buffer that could not be allocated.
#[derive(Debug)]
pub(crate) enum AllocationError {
    /// Reject a mask whose dimensions cannot form a physical raster.
    InvalidDimensions,
    /// Retain the allocator's reason for rejecting the required buffer.
    Buffer(std::collections::TryReserveError),
}

impl std::fmt::Display for AllocationError {
    /// Preserve a specific buffer-allocation failure for renderer diagnostics.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimensions => formatter.write_str("Invalid splash coverage dimensions"),
            Self::Buffer(error) => {
                write!(formatter, "Cannot allocate splash coverage buffer: {error}")
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
pub(crate) fn rounded(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    scale: f32,
) -> Option<Path> {
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
pub(crate) fn coverage(path: &Path, width: u32, height: u32) -> Result<Mask, AllocationError> {
    rasterize(path, width, height, true)
}

/// Append an adaptively flattened logical cubic with snapped physical vertices.
pub(crate) fn append_cubic(path: &mut PathBuilder, points: [[f64; 2]; 4], scale: f64) {
    corner(path, points, scale, 9);
}

/// Intersect a physical silhouette with its flat completion and console-facing cuts.
pub(crate) fn intersect(path: &Path, right: f32, bottom: f32) -> Option<Path> {
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

/// Share bounded pixel-area integration between convex fills and single stroke outlines.
fn rasterize(path: &Path, width: u32, height: u32, convex: bool) -> Result<Mask, AllocationError> {
    let polygon: Vec<[f64; 2]> = path
        .segments()
        .filter_map(|segment| match segment {
            PathSegment::MoveTo(point) | PathSegment::LineTo(point) => {
                Some([f64::from(point.x), f64::from(point.y)])
            }
            _ => None,
        })
        .collect();
    let bounds = path.bounds();
    let center = f64::from(bounds.left() + bounds.right()) * 0.5;
    let size =
        tiny_skia::IntSize::from_wh(width, height).ok_or(AllocationError::InvalidDimensions)?;
    let length =
        (width as usize).checked_mul(height as usize).ok_or(AllocationError::InvalidDimensions)?;
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(length).map_err(AllocationError::Buffer)?;
    buffer.resize(length, 0);
    let mut mask = Mask::from_vec(buffer, size).ok_or(AllocationError::InvalidDimensions)?;
    for row in
        bounds.top().floor().max(0.0) as u32..bounds.bottom().ceil().min(height as f32) as u32
    {
        let strip = raster_cut(
            &raster_cut(&polygon, 1, f64::from(row), true),
            1,
            f64::from(row + 1),
            false,
        );
        if strip.is_empty() {
            continue;
        }
        let left = strip.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min);
        let right = strip.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max);
        let inner_left = strip
            .iter()
            .filter(|point| point[0] <= center)
            .map(|point| point[0])
            .fold(left, f64::max);
        let inner_right = strip
            .iter()
            .filter(|point| point[0] >= center)
            .map(|point| point[0])
            .fold(right, f64::min);
        let top = strip.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min);
        let bottom = strip.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max);
        for column in left.floor().max(0.0) as u32..right.ceil().min(f64::from(width)) as u32 {
            let area = if convex
                && f64::from(column) >= inner_left
                && f64::from(column + 1) <= inner_right
            {
                bottom - top
            } else {
                let pixel = raster_cut(
                    &raster_cut(&strip, 0, f64::from(column), true),
                    0,
                    f64::from(column + 1),
                    false,
                );
                let mut twice_area = 0.0;
                for (index, point) in pixel.iter().enumerate() {
                    let next = pixel[(index + 1) % pixel.len()];
                    twice_area += point[0] * next[1] - point[1] * next[0];
                }
                twice_area.abs() * 0.5
            };
            mask.data_mut()[(row * width + column) as usize] =
                (area * 256.0).ceil().min(255.0) as u8;
        }
    }
    Ok(mask)
}

/// Clip a convex polygon to one axis-aligned half-plane, retaining its winding.
fn cut(polygon: &[[f64; 2]], axis: usize, bound: f64, greater: bool) -> Vec<[f64; 2]> {
    let mut output = Vec::with_capacity(polygon.len() + 2);
    for (index, &point) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        let inside = (point[axis] >= bound) == greater;
        let next_inside = (next[axis] >= bound) == greater;
        if inside {
            output.push(point);
        }
        if inside != next_inside {
            let fraction = (bound - point[axis]) / (next[axis] - point[axis]);
            output.push([
                point[0] + fraction * (next[0] - point[0]),
                point[1] + fraction * (next[1] - point[1]),
            ]);
        }
    }
    output
}

/// Partition raster cells on the painter's 24.8 grid after silhouette intersection.
fn raster_cut(polygon: &[[f64; 2]], axis: usize, bound: f64, greater: bool) -> Vec<[f64; 2]> {
    cut(polygon, axis, bound, greater)
        .into_iter()
        .map(|mut point| {
            point[1 - axis] = (point[1 - axis] * 256.0).floor() / 256.0;
            point
        })
        .collect()
}

/// Composite one independently rounded reference paint pass through a convex silhouette.
///
/// Return an allocation error before compositing when the required coverage buffer fails.
pub(crate) fn paint(
    target: &mut tiny_skia::Pixmap,
    path: &tiny_skia::Path,
    color: [u8; 4],
    outer: Option<&Mask>,
) -> Result<(), AllocationError> {
    let mask = coverage(path, target.width(), target.height())?;
    composite(target, path, color, outer, &mask);
    Ok(())
}

/// Paint a concave stroke outline as one independently rounded source-over pass.
///
/// Return an allocation error before compositing when the required coverage buffer fails.
pub(crate) fn paint_outline(
    target: &mut tiny_skia::Pixmap,
    path: &Path,
    color: [u8; 4],
) -> Result<(), AllocationError> {
    let mask = rasterize(path, target.width(), target.height(), false)?;
    composite(target, path, color, None, &mask);
    Ok(())
}

/// Composite an already prepared mask over the target's premultiplied bytes.
fn composite(
    target: &mut tiny_skia::Pixmap,
    path: &Path,
    color: [u8; 4],
    outer: Option<&Mask>,
    mask: &Mask,
) {
    let bounds = path.bounds();
    for row in bounds.top().floor().max(0.0) as u32
        ..bounds.bottom().ceil().min(target.height() as f32) as u32
    {
        for column in bounds.left().floor().max(0.0) as u32
            ..bounds.right().ceil().min(target.width() as f32) as u32
        {
            let index = (row * target.width() + column) as usize;
            let coverage = outer.map_or(mask.data()[index], |outer| {
                byte_product(mask.data()[index], outer.data()[index])
            });
            let alpha = byte_product(color[3], coverage);
            let source = [
                byte_product(byte_product(color[0], color[3]), coverage),
                byte_product(byte_product(color[1], color[3]), coverage),
                byte_product(byte_product(color[2], color[3]), coverage),
                alpha,
            ];
            let pixels = target.data_mut();
            for channel in 0..4 {
                pixels[index * 4 + channel] =
                    source[channel] + byte_product(pixels[index * 4 + channel], 255 - alpha);
            }
        }
    }
}
/// Round a byte channel's normalized coverage before source-over composition.
pub(crate) fn byte_product(channel: u8, coverage: u8) -> u8 {
    let product = u16::from(channel) * u16::from(coverage);
    ((product + (product >> 8) + 128) >> 8) as u8
}

#[cfg(test)]
mod tests {
    use super::{coverage, rounded};

    /// Preserve independently clamped radii and the native painter's thin fill coverage.
    #[test]
    fn narrow_fill_retains_reference_elliptical_ends() -> Result<(), Box<dyn std::error::Error>> {
        for (scale, rows) in [
            (1.0, vec![[153, 0], [231, 0], [231, 0], [153, 0]]),
            (1.5, vec![[159, 39], [217, 88], [243, 115], [243, 115], [217, 88], [159, 39]]),
        ] {
            let path =
                rounded(0.0, 0.0, 1.0, 4.0, 2.0, scale).ok_or("Cannot construct thin fill")?;
            let mask = coverage(&path, 2, 6)?;
            for (row, expected) in rows.into_iter().enumerate() {
                for (actual, expected) in mask.data()[row * 2..row * 2 + 2].iter().zip(expected) {
                    assert!(actual.abs_diff(expected) <= 1);
                    if expected == 0 {
                        assert_eq!(*actual, 0);
                    }
                }
            }
        }
        Ok(())
    }
    /// Report an unavailable required buffer instead of claiming a partial frame succeeded.
    #[test]
    fn coverage_allocation_failure_is_observable() -> Result<(), Box<dyn std::error::Error>> {
        let path =
            rounded(0.0, 0.0, 1.0, 4.0, 2.0, 1.0).ok_or("Cannot construct allocation fixture")?;
        for (width, height) in [(0, 1), (1, 0), (u32::MAX, u32::MAX)] {
            assert!(coverage(&path, width, height).is_err());
        }
        let error =
            coverage(&path, u32::MAX, u32::MAX).err().ok_or("Allocation unexpectedly succeeded")?;
        assert!(std::error::Error::source(&error).is_some());
        Ok(())
    }
}
