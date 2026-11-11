// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The convex boundary of a coverage's occupied pixels, as CuteCanvas frames a mask layer.
//!
//! A pixel is occupied at or above a threshold. Each row with occupied pixels contributes the
//! pixel-edge corners of its leftmost and rightmost occupied runs; the boundary is the convex
//! hull of those corners by Andrew's monotone chain, dropping collinear points, then simplified
//! by repeatedly removing the first vertex within 1.25 pixels of the line through its
//! neighbours while more than three remain. The polygon starts at its smallest x (then y),
//! has positive shoelace area in y-down coordinates and is not closed. Content kept beyond the
//! extent is included, in the coverage's own signed coordinates.
use super::{Coverage, Slot, tile_index};
use crate::TILE_EDGE;
use std::collections::BTreeMap;

/// Largest distance, in pixels, a vertex may lie from its neighbours' line and still be dropped.
const SIMPLIFICATION: f64 = 1.25;

impl Coverage {
    /// The simplified convex boundary of pixels holding at least `threshold`, or an empty
    /// polygon when fewer than three distinct corners exist.
    pub fn convex_boundary(&self, threshold: u16) -> Vec<[f64; 2]> {
        let mut rows = BTreeMap::new();
        self.row_extents(threshold, [0, 0], &mut rows);
        if let Some((frame, origin)) = self.outside() {
            frame.row_extents(threshold, origin, &mut rows);
        }
        let mut points: Vec<[i64; 2]> = rows
            .iter()
            .flat_map(|(row, (left, right))| {
                [[*left, *row], [*right, *row], [*right, row + 1], [*left, row + 1]]
            })
            .collect();
        points.sort_unstable();
        points.dedup();
        let hull = hull(&points);
        simplify(hull.into_iter().map(|[x, y]| [x as f64, y as f64]).collect())
    }

    /// Merge each row's occupied `[left, right)` pixel-edge span into `rows`.
    fn row_extents(
        &self,
        threshold: u16,
        [dx, dy]: [i64; 2],
        rows: &mut BTreeMap<i64, (i64, i64)>,
    ) {
        let mut include = |row: u32, left: u32, right: u32| {
            let (row, left, right) =
                (i64::from(row) + dy, i64::from(left) + dx, i64::from(right) + dx);
            let span = rows.entry(row).or_insert((left, right));
            *span = (span.0.min(left), span.1.max(right));
        };
        let region = self.extent.region();
        for part in region.tiles() {
            let key = self.key(part.x() / TILE_EDGE, part.y() / TILE_EDGE);
            match self.tiles.slots.get(&key) {
                None if self.tiles.default >= threshold => {
                    (part.y()..part.bottom()).for_each(|y| include(y, part.x(), part.right()));
                }
                Some(Slot::Uniform(value)) if *value >= threshold => {
                    (part.y()..part.bottom()).for_each(|y| include(y, part.x(), part.right()));
                }
                Some(Slot::Stored(tile)) => {
                    for y in part.y()..part.bottom() {
                        let start = tile_index(part.x(), y);
                        let line = &tile.values[start..start + part.width() as usize];
                        let first = line.iter().position(|value| *value >= threshold);
                        let last = line.iter().rposition(|value| *value >= threshold);
                        if let (Some(first), Some(last)) = (first, last) {
                            include(y, part.x() + first as u32, part.x() + last as u32 + 1);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Andrew's monotone chain over sorted unique points; collinear points are dropped.
fn hull(points: &[[i64; 2]]) -> Vec<[i64; 2]> {
    if points.len() < 3 {
        return Vec::new();
    }
    let cross = |o: [i64; 2], a: [i64; 2], b: [i64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let chain = |points: &mut dyn Iterator<Item = [i64; 2]>| {
        let mut chain: Vec<[i64; 2]> = Vec::new();
        for point in points {
            while chain.len() >= 2
                && cross(chain[chain.len() - 2], chain[chain.len() - 1], point) <= 0
            {
                chain.pop();
            }
            chain.push(point);
        }
        chain
    };
    let mut lower = chain(&mut points.iter().copied());
    let mut upper = chain(&mut points.iter().rev().copied());
    lower.pop();
    upper.pop();
    lower.extend(upper);
    if lower.len() < 3 { Vec::new() } else { lower }
}

/// Remove the first vertex close to its neighbours' line, repeatedly, while more than three remain.
fn simplify(mut polygon: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    while polygon.len() > 3 {
        let count = polygon.len();
        let removable = (0..count).find(|index| {
            let previous = polygon[(index + count - 1) % count];
            let next = polygon[(index + 1) % count];
            line_distance(polygon[*index], previous, next) <= SIMPLIFICATION
        });
        match removable {
            Some(index) => {
                polygon.remove(index);
            }
            None => break,
        }
    }
    polygon
}

/// Distance from `point` to the infinite line through `start` and `end`; infinite for a line
/// shorter than 1e-12.
fn line_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let [dx, dy] = [end[0] - start[0], end[1] - start[1]];
    let length = dx.hypot(dy);
    if length <= 1e-12 {
        return f64::INFINITY;
    }
    ((point[0] - start[0]) * dy - (point[1] - start[1]) * dx).abs() / length
}
