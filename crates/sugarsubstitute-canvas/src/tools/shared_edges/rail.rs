// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Snap a pivoting seam endpoint along its rail, as CuteCanvas's rail snap resolver does: to
//! crossings with other edges, to 0°, 45° and 90° lines through the seam's fixed end, and to
//! continuations of edges through that end, within 6 device pixels until 9 away.
use super::{
    edges::{Edge, Owner, Point, add, cross, distance, dot, scale, sub},
    gesture::{Pivot, RELEASE, SNAP},
};

/// One rail snap target.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RailTarget {
    point: Point,
    owner: RailOwner,
    priority: i32,
}

/// What a rail target lies on, ordered as CuteCanvas's owner names order ties.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RailOwner {
    Edge(Owner),
    Orientation(u8),
}

/// Rail snapping for one endpoint drag.
#[derive(Debug)]
pub(super) struct RailSnap {
    start: Point,
    direction: Point,
    length: f64,
    targets: Vec<RailTarget>,
    lock: Option<RailTarget>,
}

impl RailSnap {
    pub(super) fn new(pivot: &Pivot, edges: &[Edge]) -> Option<Self> {
        let [start, end] = pivot.rail();
        let length = distance(start, end);
        if length <= 1e-12 {
            return None;
        }
        let rail = sub(end, start);
        let direction = scale(rail, 1.0 / length);
        let mut targets: Vec<RailTarget> = edges
            .iter()
            .filter_map(|edge| {
                let along = sub(edge.end, edge.start);
                let denominator = cross(rail, along);
                if denominator.abs() <= 1e-9 * length * edge.length() {
                    return None;
                }
                let offset = sub(edge.start, start);
                let (on_rail, on_edge) =
                    (cross(offset, along) / denominator, cross(offset, rail) / denominator);
                ((0.0..=1.0).contains(&on_rail) && (0.0..=1.0).contains(&on_edge)).then(|| {
                    RailTarget {
                        point: add(start, scale(rail, on_rail)),
                        owner: RailOwner::Edge(edge.owner),
                        priority: edge.priority,
                    }
                })
            })
            .collect();
        for (index, line) in
            [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [1.0, -1.0]].into_iter().enumerate()
        {
            if let Some(point) = line_crossing(start, end, pivot.fixed(), line)
                && distance(point, pivot.fixed()) > 1e-9
            {
                // CuteCanvas names both diagonals "orientation:45".
                let owner = RailOwner::Orientation(if index == 3 { 1 } else { index as u8 });
                targets.push(RailTarget { point, owner, priority: 0 });
            }
        }
        for edge in edges {
            if let Some(target) = continuation(start, end, pivot.fixed(), edge) {
                targets.push(target);
            }
        }
        Some(Self { start, direction, length, targets, lock: None })
    }

    pub(super) fn resolve(&mut self, pointer: Point, scale: f64, suppressed: bool) -> Point {
        let projection = dot(sub(pointer, self.start), self.direction).clamp(0.0, self.length);
        let raw = add(self.start, self::scale(self.direction, projection));
        if suppressed {
            self.lock = None;
            return raw;
        }
        if let Some(lock) = self.lock {
            let held = self.targets.iter().find(|target| {
                target.owner == lock.owner && distance(target.point, lock.point) <= 1e-8
            });
            if let Some(held) = held
                && distance(raw, held.point) <= RELEASE * scale
            {
                return held.point;
            }
            self.lock = None;
        }
        let best = self
            .targets
            .iter()
            .filter(|target| distance(raw, target.point) <= SNAP * scale)
            .min_by(|a, b| {
                distance(raw, a.point)
                    .total_cmp(&distance(raw, b.point))
                    .then(b.priority.cmp(&a.priority))
                    .then(a.owner.cmp(&b.owner))
                    .then(a.point[0].total_cmp(&b.point[0]))
                    .then(a.point[1].total_cmp(&b.point[1]))
            })
            .copied();
        match best {
            Some(target) => {
                self.lock = Some(target);
                target.point
            }
            None => raw,
        }
    }
}

/// Where the line through `point` along `direction` crosses the rail, inside it.
fn line_crossing(start: Point, end: Point, point: Point, direction: Point) -> Option<Point> {
    let rail = sub(end, start);
    let denominator = cross(rail, direction);
    let size = rail[0].hypot(rail[1]) * direction[0].hypot(direction[1]);
    if denominator.abs() <= 1e-9 * size {
        return None;
    }
    let parameter = cross(sub(point, start), direction) / denominator;
    (0.0..=1.0).contains(&parameter).then(|| add(start, scale(rail, parameter)))
}

/// A rail target continuing an edge whose line passes through the fixed end without a gap.
fn continuation(start: Point, end: Point, fixed: Point, edge: &Edge) -> Option<RailTarget> {
    let tangent = edge.tangent();
    if cross(sub(fixed, edge.start), tangent).abs() > 1e-7 {
        return None;
    }
    let point = line_crossing(start, end, edge.start, tangent)?;
    if distance(point, fixed) <= 1e-9 {
        return None;
    }
    let mut seam = [dot(tangent, fixed), dot(tangent, point)];
    seam.sort_by(f64::total_cmp);
    let own = edge.interval();
    let gap = (own[0] - seam[1]).max(seam[0] - own[1]).max(0.0);
    (gap <= 1e-7).then_some(RailTarget {
        point,
        owner: RailOwner::Edge(edge.owner),
        priority: edge.priority,
    })
}
