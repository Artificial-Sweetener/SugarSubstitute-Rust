// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Resume the reference pixel-area integration without changing its evaluation order.

use super::AllocationError;
use crate::bitmap::CoverageMask;
use tiny_skia::{IntSize, Path, PathSegment};

/// Retain one row's clipped polygon and exact interior-area shortcut across steps.
struct Row {
    strip: Vec<[f64; 2]>,
    left: u32,
    right: u32,
    inner_left: f64,
    inner_right: f64,
    area: f64,
}

/// Own unpublished coverage with bounded zero initialization and pixel integration.
pub struct CoverageRaster {
    polygon: Vec<[f64; 2]>,
    size: IntSize,
    center: f64,
    convex: bool,
    row: u32,
    bottom: u32,
    current: Option<Row>,
    buffer: Vec<u8>,
}

impl CoverageRaster {
    /// Reserve a full physical coverage plane without zeroing or rasterizing it yet.
    pub fn new(
        path: &Path,
        width: u32,
        height: u32,
        convex: bool,
    ) -> Result<Self, AllocationError> {
        let size = IntSize::from_wh(width, height).ok_or(AllocationError::InvalidDimensions)?;
        let length = (width as usize)
            .checked_mul(height as usize)
            .ok_or(AllocationError::InvalidDimensions)?;
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(length).map_err(AllocationError::Buffer)?;
        let points = path.segments().filter_map(|segment| match segment {
            PathSegment::MoveTo(point) | PathSegment::LineTo(point) => {
                Some([f64::from(point.x), f64::from(point.y)])
            }
            _ => None,
        });
        let polygon: Vec<_> = points.collect();
        // Preserve reference partition rounding for already admitted silhouettes.
        // Compact redundant edges only when needed to satisfy the same vertex bound.
        let polygon =
            if polygon.len() > 128 { compact_polygon(polygon.into_iter()) } else { polygon };
        if polygon.len() > 128 || buffer.capacity() != length {
            return Err(AllocationError::InvalidDimensions);
        }
        let bounds = path.bounds();
        Ok(Self {
            polygon,
            size,
            center: f64::from(bounds.left() + bounds.right()) * 0.5,
            convex,
            row: bounds.top().floor().max(0.0) as u32,
            bottom: bounds.bottom().ceil().min(height as f32) as u32,
            current: None,
            buffer,
        })
    }

    /// Initialize/integrate at most `max` pixels and visit at most sixteen new rows.
    ///
    /// Pixel-area arithmetic, winding, clipping and 24.8 snapping are unchanged by
    /// a yield. Each call clamps work to 65536 cells; row state survives a partial row.
    pub fn step(&mut self, max: usize) -> bool {
        let max = max.clamp(1, 65536);
        let length = self.size.width() as usize * self.size.height() as usize;
        if self.buffer.len() < length {
            self.buffer.resize(length.min(self.buffer.len() + max), 0);
            return false;
        }
        let mut pixels = 0;
        let mut rows = 0;
        while self.row < self.bottom && pixels < max && rows < 16 {
            if self.current.is_none() {
                rows += 1;
                self.current = self.prepare_row();
                if self.current.is_none() {
                    self.row += 1;
                    continue;
                }
            }
            if let Some(row) = &mut self.current {
                let end = row.right.min(row.left.saturating_add((max - pixels) as u32));
                for column in row.left..end {
                    let area = if self.convex
                        && f64::from(column) >= row.inner_left
                        && f64::from(column + 1) <= row.inner_right
                    {
                        row.area
                    } else {
                        let pixel = raster_cut(
                            &raster_cut(&row.strip, 0, f64::from(column), true),
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
                    self.buffer[(self.row * self.size.width() + column) as usize] =
                        (area * 256.0).ceil().min(255.0) as u8;
                }
                pixels += (end - row.left) as usize;
                row.left = end;
                if end == row.right {
                    self.current = None;
                    self.row += 1;
                }
            }
        }
        self.row >= self.bottom
    }

    /// Transfer fully initialized coverage without copying or scanning pixels.
    pub fn finish(self) -> Result<CoverageMask, AllocationError> {
        if self.row < self.bottom || self.current.is_some() {
            return Err(AllocationError::InvalidDimensions);
        }
        CoverageMask::from_vec(self.buffer, self.size).ok_or(AllocationError::InvalidDimensions)
    }

    /// Clip one physical row with the original f64 integration and shortcut bounds.
    fn prepare_row(&self) -> Option<Row> {
        let strip = raster_cut(
            &raster_cut(&self.polygon, 1, f64::from(self.row), true),
            1,
            f64::from(self.row + 1),
            false,
        );
        if strip.is_empty() {
            return None;
        }
        let left = strip.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min);
        let right = strip.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max);
        let inner_left = strip
            .iter()
            .filter(|point| point[0] <= self.center)
            .map(|point| point[0])
            .fold(left, f64::max);
        let inner_right = strip
            .iter()
            .filter(|point| point[0] >= self.center)
            .map(|point| point[0])
            .fold(right, f64::min);
        let top = strip.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min);
        let bottom = strip.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max);
        let left = left.floor().max(0.0).min(f64::from(self.size.width())) as u32;
        let right = right.ceil().min(f64::from(self.size.width())).max(0.0) as u32;
        if left >= right {
            return None;
        }
        Some(Row { strip, left, right, inner_left, inner_right, area: bottom - top })
    }
}

/// Remove only zero-length and exactly collinear forward segments after grid snapping.
/// Their pixel coverage is identical; reversals retain their winding contribution.
fn compact_polygon(points: impl Iterator<Item = [f64; 2]>) -> Vec<[f64; 2]> {
    let mut polygon: Vec<[f64; 2]> = Vec::new();
    for point in points {
        if polygon.last() == Some(&point) {
            continue;
        }
        while polygon.len() >= 2 {
            let a = polygon[polygon.len() - 2];
            let b = polygon[polygon.len() - 1];
            let ab = [b[0] - a[0], b[1] - a[1]];
            let bc = [point[0] - b[0], point[1] - b[1]];
            if ab[0] * bc[1] != ab[1] * bc[0] || ab[0] * bc[0] + ab[1] * bc[1] < 0.0 {
                break;
            }
            polygon.pop();
        }
        polygon.push(point);
    }
    polygon
}

/// Clip one axis-aligned half-plane while retaining the original polygon winding.
pub(super) fn cut(polygon: &[[f64; 2]], axis: usize, bound: f64, greater: bool) -> Vec<[f64; 2]> {
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

/// Snap the secondary coordinate after each cell partition, preserving reference rounding.
fn raster_cut(polygon: &[[f64; 2]], axis: usize, bound: f64, greater: bool) -> Vec<[f64; 2]> {
    cut(polygon, axis, bound, greater)
        .into_iter()
        .map(|mut point| {
            point[1 - axis] = (point[1 - axis] * 256.0).floor() / 256.0;
            point
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polygon_compaction_removes_only_redundant_forward_vertices() {
        let points = [
            [0.0, 0.0],
            [1.0, 1.0],
            [1.0, 1.0],
            [2.0, 2.0],
            [1.0, 1.0],
            [2.0, 1.0],
            [3.0, 1.0],
            [3.0, 2.0],
        ];
        assert_eq!(
            compact_polygon(points.into_iter()),
            [[0.0, 0.0], [2.0, 2.0], [1.0, 1.0], [3.0, 1.0], [3.0, 2.0]]
        );
    }
    /// A one-cell budget also bounds full-plane initialization, retaining exact native coverage.
    #[test]
    fn initialization_and_area_integration_obey_the_requested_quantum()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = crate::shapes::rounded(1.0, 1.0, 1.0, 4.0, 2.0, 1.0)
            .ok_or("Missing thin-fill fixture")?;
        let mut raster = CoverageRaster::new(&path, 8, 8, true)?;
        for length in 1..=64 {
            assert!(!raster.step(1));
            assert_eq!(raster.buffer.len(), length);
            assert!(raster.current.is_none());
        }
        loop {
            let previous = raster.buffer.clone();
            let done = raster.step(1);
            assert!(previous.iter().zip(&raster.buffer).filter(|(a, b)| a != b).count() <= 1);
            if done {
                break;
            }
        }
        let mask = raster.finish()?;
        assert_eq!(
            [mask.data()[9], mask.data()[17], mask.data()[25], mask.data()[33]],
            [153, 231, 231, 153]
        );
        Ok(())
    }
}
