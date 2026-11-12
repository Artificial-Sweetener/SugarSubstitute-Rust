// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed document-space outlines and their exact-area anti-aliased rasterization.
//!
//! Every vector shape that produces coverage (selection shapes, mask shapes, lassos) is a
//! [`Path`] rasterized here. Pixel `(x, y)` covers the square `[x, x + 1) × [y, y + 1)`. Coverage
//! is the exact signed area under the outline, accumulated per row and clamped to one, so
//! overlapping contours and self-intersecting lassos fill with nonzero winding.
use super::{Coverage, FULL_COVERAGE};
use crate::{Extent, ImageError, Region, TILE_EDGE, check};
use sugarsubstitute_contracts::execution::JobContext;

const MAX_POINTS: usize = 1 << 20;

/// One or more closed polygons in document pixel coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    contours: Vec<Vec<[f64; 2]>>,
}

impl Path {
    /// A closed polygon through `points`; the last point connects back to the first.
    /// # Errors
    /// Rejects fewer than three points, nonfinite coordinates or an excessive point count.
    pub fn polygon(points: Vec<[f64; 2]>) -> Result<Self, ImageError> {
        if points.len() < 3
            || points.len() > MAX_POINTS
            || points.iter().flatten().any(|value| !value.is_finite())
        {
            return Err(ImageError::Invalid("Invalid coverage polygon"));
        }
        Ok(Self { contours: vec![points] })
    }

    /// An axis-aligned rectangle with edges `[left, top, right, bottom]`.
    /// # Errors
    /// Rejects nonfinite or empty bounds.
    pub fn rectangle([left, top, right, bottom]: [f64; 4]) -> Result<Self, ImageError> {
        if !(left < right && top < bottom) {
            return Err(ImageError::Invalid("Invalid coverage rectangle"));
        }
        Self::polygon(vec![[left, top], [right, top], [right, bottom], [left, bottom]])
    }

    /// The ellipse inscribed in `[left, top, right, bottom]`, flattened finely enough that its
    /// polygon stays within a thousandth of a pixel of the true curve.
    /// # Errors
    /// Rejects nonfinite or empty bounds.
    pub fn ellipse([left, top, right, bottom]: [f64; 4]) -> Result<Self, ImageError> {
        if !(left < right && top < bottom) {
            return Err(ImageError::Invalid("Invalid coverage ellipse"));
        }
        let center = [(left + right) / 2.0, (top + bottom) / 2.0];
        let radii = [(right - left) / 2.0, (bottom - top) / 2.0];
        // Chord error r(1 - cos(θ/2)) ≤ 0.001 gives θ ≤ 2·acos(1 - 0.001/r).
        let radius = radii[0].max(radii[1]);
        let step = 2.0 * (1.0 - (0.001 / radius).min(1.0)).acos();
        let segments = ((std::f64::consts::TAU / step).ceil() as usize).clamp(16, 1 << 16);
        let points = (0..segments)
            .map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / segments as f64;
                [center[0] + radii[0] * angle.cos(), center[1] + radii[1] * angle.sin()]
            })
            .collect();
        Self::polygon(points)
    }

    /// Bounding box `[left, top, right, bottom]` of every point.
    pub fn bounds(&self) -> [f64; 4] {
        self.contours.iter().flatten().fold(
            [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY],
            |[l, t, r, b], [x, y]| [l.min(*x), t.min(*y), r.max(*x), b.max(*y)],
        )
    }

    /// Translate every point by `[dx, dy]`.
    pub fn translated(&self, [dx, dy]: [f64; 2]) -> Self {
        Self {
            contours: self
                .contours
                .iter()
                .map(|contour| contour.iter().map(|[x, y]| [x + dx, y + dy]).collect())
                .collect(),
        }
    }

    /// Rasterize into new coverage over `extent`, empty outside the outline.
    /// # Errors
    /// Preserves cancellation, deadlines and memory pressure.
    pub fn rasterize(&self, extent: Extent, context: &JobContext) -> Result<Coverage, ImageError> {
        let memory = context.memory();
        let mut coverage = Coverage::new(extent, 0, memory)?;
        let [left, top, right, bottom] = self.bounds();
        let clip = |value: f64, limit: u32| value.clamp(0.0, f64::from(limit));
        let (left, right) = (clip(left, extent.width()), clip(right, extent.width()));
        let (top, bottom) = (clip(top, extent.height()), clip(bottom, extent.height()));
        if left >= right || top >= bottom {
            return Ok(coverage);
        }
        let first_column = left.floor() as u32;
        let width = (right.ceil() as u32 - first_column).max(1);
        let mut accumulation = Vec::new();
        accumulation.try_reserve_exact((width as usize + 2) * TILE_EDGE as usize)?;
        let _scratch = memory.claim(((width as u64 + 2) * u64::from(TILE_EDGE) + 64) * 4)?;
        let mut band_top = top.floor() as u32 / TILE_EDGE * TILE_EDGE;
        while f64::from(band_top) < bottom {
            check(context)?;
            let band_height = TILE_EDGE.min(extent.height() - band_top);
            let stride = width as usize + 2;
            accumulation.clear();
            accumulation.resize(stride * band_height as usize, 0.0_f32);
            let origin = [f64::from(first_column), f64::from(band_top)];
            for contour in &self.contours {
                for (index, start) in contour.iter().enumerate() {
                    let end = contour[(index + 1) % contour.len()];
                    for [from, to] in clip_horizontally(*start, end, [left, right]) {
                        accumulate(
                            &mut accumulation,
                            stride,
                            band_height,
                            [from[0] - origin[0], from[1] - origin[1]],
                            [to[0] - origin[0], to[1] - origin[1]],
                        );
                    }
                }
            }
            for row in accumulation.chunks_exact_mut(stride) {
                let mut sum = 0.0_f32;
                for cell in row {
                    sum += *cell;
                    *cell = sum.abs().min(1.0);
                }
            }
            let region = Region::new(first_column, band_top, width, band_height)?;
            coverage.update(region, memory, |x, y, _| {
                let cell = (y - band_top) as usize * stride + (x - first_column) as usize;
                (accumulation[cell] * f32::from(FULL_COVERAGE)).round() as u16
            })?;
            band_top += band_height;
        }
        Ok(coverage)
    }
}

/// Split a segment where it crosses the left or right clip edge, pinning the outside parts to
/// that edge. Area to the right of an edge is unchanged inside the clip by moving the outside
/// part onto the boundary, so clipped outlines rasterize exactly within `[left, right]`.
fn clip_horizontally(
    start: [f64; 2],
    end: [f64; 2],
    [left, right]: [f64; 2],
) -> Vec<[[f64; 2]; 2]> {
    let mut cuts = vec![0.0, 1.0];
    for edge in [left, right] {
        let span = end[0] - start[0];
        if span != 0.0 {
            let at = (edge - start[0]) / span;
            if at > 0.0 && at < 1.0 {
                cuts.push(at);
            }
        }
    }
    cuts.sort_by(f64::total_cmp);
    let point = |at: f64| {
        let x = start[0] + (end[0] - start[0]) * at;
        [x.clamp(left, right), start[1] + (end[1] - start[1]) * at]
    };
    cuts.windows(2).map(|pair| [point(pair[0]), point(pair[1])]).collect()
}

/// Add the signed area one edge contributes to every pixel it crosses in a band. A row's prefix
/// sum then yields each pixel's covered area. Coordinates are relative to the band's origin and
/// lie within `[0, stride - 2]` horizontally.
///
/// Within one row slice of signed height `h`, column `c` is covered by the mean over the slice
/// of `clamp(c + 1 - x(t), 0, 1)`, which is `G(c + 1) - G(c)` for `G(b) = ∫ max(0, b - x(t)) dt`.
/// Each cell receives the difference between its column's coverage and the previous column's.
fn accumulate(cells: &mut [f32], stride: usize, height: u32, from: [f64; 2], to: [f64; 2]) {
    if from[1] == to[1] {
        return;
    }
    let (direction, from, to) = if from[1] < to[1] { (1.0, from, to) } else { (-1.0, to, from) };
    let slope = (to[0] - from[0]) / (to[1] - from[1]);
    let first = from[1].max(0.0).floor() as u32;
    let last = to[1].min(f64::from(height)).ceil() as u32;
    for y in first..last {
        let top = f64::from(y).max(from[1]);
        let bottom = f64::from(y + 1).min(to[1]);
        if bottom <= top {
            continue;
        }
        let slice = (bottom - top) * direction;
        let x_top = from[0] + (top - from[1]) * slope;
        let x_bottom = from[0] + (bottom - from[1]) * slope;
        let (x0, x1) = if x_top < x_bottom { (x_top, x_bottom) } else { (x_bottom, x_top) };
        let area_left_of = |boundary: f64| {
            if boundary <= x0 {
                0.0
            } else if boundary >= x1 {
                boundary - (x0 + x1) / 2.0
            } else {
                (boundary - x0) * (boundary - x0) / (2.0 * (x1 - x0))
            }
        };
        let row = y as usize * stride;
        let mut previous = 0.0;
        for column in x0.floor() as usize..=x1.ceil() as usize {
            let left = column as f64;
            let covered = area_left_of(left + 1.0) - area_left_of(left);
            cells[row + column] += (slice * (covered - previous)) as f32;
            previous = covered;
        }
    }
}
