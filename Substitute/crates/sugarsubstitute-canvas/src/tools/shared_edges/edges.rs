// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Oriented polygon edges and the plane geometry shared-edge resizing measures with, as
//! CuteCanvas's edge model defines them.
//!
//! An edge's tangent is flipped to point right, or down when vertical, so parallel edges share
//! one tangent; its normal is the tangent turned a quarter, and its line offset is the normal's
//! projection of the start. All coordinates are document pixels.
use crate::LayerId;

/// A document point.
pub(super) type Point = [f64; 2];

/// Shortest edge length treated as a point.
const EPSILON: f64 = 1e-12;

/// What an edge belongs to; orders snap ties as CuteCanvas orders owner names.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Owner {
    /// A layer of the document.
    Layer(LayerId),
    /// The document's canvas rectangle.
    Canvas,
}

/// One polygon edge with its owner's vertex centroid and snap priority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Edge {
    pub(super) owner: Owner,
    pub(super) start: Point,
    pub(super) end: Point,
    pub(super) center: Point,
    pub(super) priority: i32,
}
impl Edge {
    pub(super) fn length(&self) -> f64 {
        distance(self.start, self.end)
    }
    pub(super) fn tangent(&self) -> Point {
        let length = self.length();
        let tangent =
            [(self.end[0] - self.start[0]) / length, (self.end[1] - self.start[1]) / length];
        if tangent[0] < -EPSILON || (tangent[0].abs() <= EPSILON && tangent[1] < 0.0) {
            [-tangent[0], -tangent[1]]
        } else {
            tangent
        }
    }
    pub(super) fn normal(&self) -> Point {
        let [x, y] = self.tangent();
        [-y, x]
    }
    pub(super) fn offset(&self) -> f64 {
        dot(self.normal(), self.start)
    }
    /// The edge's extent along its tangent.
    pub(super) fn interval(&self) -> [f64; 2] {
        let tangent = self.tangent();
        sorted([dot(tangent, self.start), dot(tangent, self.end)])
    }
    /// The same edge moved `distance` along its normal.
    pub(super) fn translated(&self, distance: f64) -> Self {
        let delta = scale(self.normal(), distance);
        Self {
            start: add(self.start, delta),
            end: add(self.end, delta),
            center: add(self.center, delta),
            ..*self
        }
    }
}

/// Whether two edges' tangents are parallel within CuteCanvas's sine tolerance.
pub(super) fn parallel(first: &Edge, second: &Edge) -> bool {
    cross(first.tangent(), second.tangent()).abs() <= 1e-4
}

/// The edges of a closed polygon, coincident points merged, sharing their vertices' centroid.
pub(super) fn polygon_edges(owner: Owner, boundary: &[Point], priority: i32) -> Vec<Edge> {
    let mut points: Vec<Point> = Vec::new();
    for point in boundary {
        if points.last().is_none_or(|last| distance(*last, *point) > EPSILON) {
            points.push(*point);
        }
    }
    if points.len() > 1 && distance(points[0], points[points.len() - 1]) <= EPSILON {
        points.pop();
    }
    if points.len() < 3 {
        return Vec::new();
    }
    let center = centroid(&points);
    (0..points.len())
        .map(|index| Edge {
            owner,
            start: points[index],
            end: points[(index + 1) % points.len()],
            center,
            priority,
        })
        .collect()
}

/// The edges of an axis-aligned rectangle `[left, top, right, bottom]`.
pub(super) fn rectangle_edges(
    owner: Owner,
    [left, top, right, bottom]: [f64; 4],
    priority: i32,
) -> Vec<Edge> {
    polygon_edges(owner, &[[left, top], [right, top], [right, bottom], [left, bottom]], priority)
}

pub(super) fn centroid(points: &[Point]) -> Point {
    let count = points.len().max(1) as f64;
    let [x, y] = points.iter().fold([0.0, 0.0], |sum, point| add(sum, *point));
    [x / count, y / count]
}

/// Twice the signed shoelace area.
pub(super) fn signed_area_twice(points: &[Point]) -> f64 {
    (0..points.len()).map(|index| cross(points[index], points[(index + 1) % points.len()])).sum()
}

/// Whether `point` lies on the infinite line through `start` and `end`.
pub(super) fn on_line(point: Point, start: Point, end: Point, tolerance: f64) -> bool {
    let edge = sub(end, start);
    let length = edge[0].hypot(edge[1]);
    if length <= tolerance {
        return false;
    }
    cross(edge, sub(point, start)).abs() <= tolerance * length.max(1.0)
}

/// Whether `point` lies on the segment from `start` to `end`, within `tolerance`.
pub(super) fn on_segment(point: Point, start: Point, end: Point, tolerance: f64) -> bool {
    let edge = sub(end, start);
    let length = edge[0].hypot(edge[1]).max(1.0);
    if cross(edge, sub(point, start)).abs() > tolerance * length {
        return false;
    }
    let parameter = parameter(point, start, end);
    (-tolerance / length..=1.0 + tolerance / length).contains(&parameter)
}

/// Where `point` projects along the segment from `start` (0) to `end` (1).
pub(super) fn parameter(point: Point, start: Point, end: Point) -> f64 {
    let edge = sub(end, start);
    dot(sub(point, start), edge) / dot(edge, edge)
}

/// Distance from `point` to the segment from `start` to `end`.
pub(super) fn segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let segment = sub(end, start);
    let length = dot(segment, segment);
    if length <= 1e-9 {
        return distance(point, start);
    }
    let fraction = (dot(sub(point, start), segment) / length).clamp(0.0, 1.0);
    distance(point, add(start, scale(segment, fraction)))
}

/// The smallest distance between two segments' endpoints and the other segment.
pub(super) fn segments_distance(a: [Point; 2], b: [Point; 2]) -> f64 {
    [
        segment_distance(a[0], b[0], b[1]),
        segment_distance(a[1], b[0], b[1]),
        segment_distance(b[0], a[0], a[1]),
        segment_distance(b[1], a[0], a[1]),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

pub(super) fn distance(a: Point, b: Point) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
pub(super) const fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
pub(super) const fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}
pub(super) const fn scale(a: Point, factor: f64) -> Point {
    [a[0] * factor, a[1] * factor]
}
pub(super) const fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
pub(super) const fn cross(a: Point, b: Point) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn sorted([a, b]: [f64; 2]) -> [f64; 2] {
    if a <= b { [a, b] } else { [b, a] }
}
