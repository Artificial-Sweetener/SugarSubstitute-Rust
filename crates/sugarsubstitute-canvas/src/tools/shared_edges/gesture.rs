// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move a seam, as CuteCanvas's shared-edge gesture session does.
//!
//! - **Middle.** The pointer's travel along the seam's normal moves every vertex of each
//!   participant's moving chain, snapping to parallel edges of other layers and the canvas
//!   within 6 device pixels until they are 9 away; Ctrl suppresses snapping. A move that
//!   reverses a participant's winding or leaves it thinner than 2 device pixels, or than it
//!   already was, is cut back to the farthest valid fraction found in 24 halvings.
//! - **Endpoints.** The endpoint slides along the rail every participant's adjacent outer edge
//!   shares, snapping to edge crossings, to 0°, 45° and 90° lines through the fixed end and to
//!   continuations of edges through it, and joining a rail end within 6 device pixels. A
//!   participant whose edge collapses to a point is mapped bilinearly.
//!
//! Each participant's source boundary maps onto its moved boundary: by one affine map when one
//! carries every vertex, otherwise piecewise.
use super::{
    collapse::joined_mapping,
    edges::{
        Edge, Owner, Point, add, cross, distance, dot, parallel, scale, signed_area_twice, sub,
    },
    rail::RailSnap,
    seams::{Handle, Participant, Seam, chain_length, clearance},
};
use crate::{CanvasError, LayerId, SnapGuide};
use sugarsubstitute_image_ops::{CoverageMapping, PiecewiseMapping, triangle_affine};

/// Device pixels within which a snap takes hold.
pub(super) const SNAP: f64 = 6.0;
/// Device pixels within which a held snap stays.
pub(super) const RELEASE: f64 = 9.0;
/// Halvings searching for the farthest valid move.
const SEARCH_STEPS: usize = 24;

/// New mappings of the participants a gesture changed.
pub(super) type Mappings = Vec<(LayerId, CoverageMapping)>;

/// A whole-edge move snapping to parallel edges.
#[derive(Debug)]
struct EdgeSnap {
    source: Edge,
    targets: Vec<Edge>,
    lock: Option<(Owner, f64)>,
}

/// A rail-constrained endpoint drag.
#[derive(Debug)]
pub(super) struct Pivot {
    fixed: Point,
    rail: [Point; 2],
    participants: Vec<Participant>,
    corners: Vec<usize>,
}

/// One seam drag: the seam, the gripped handle and its snapping.
#[derive(Debug)]
pub(super) struct Gesture {
    pub(super) seam: Seam,
    pub(super) handle: Handle,
    origin: Point,
    pivot: Option<Pivot>,
    edge_snap: Option<EdgeSnap>,
    rail_snap: Option<RailSnap>,
    pub(super) ends: [Point; 2],
    /// The smart guide of the snap the latest placement holds.
    guide: Option<SnapGuide>,
}

impl Gesture {
    /// Begin dragging `handle` of `seam` from document point `origin`; snap targets are the
    /// non-participant `targets`. Returns `None` for a handle that cannot move.
    pub(super) fn begin(
        seam: Seam,
        handle: Handle,
        pivot: Option<Pivot>,
        origin: Point,
        targets: Vec<Edge>,
    ) -> Option<Self> {
        if (handle == Handle::Middle && !seam.translatable())
            || (handle != Handle::Middle && pivot.is_none())
        {
            return None;
        }
        let edge_snap = (handle == Handle::Middle).then(|| EdgeSnap {
            source: seam.edge,
            targets: targets.clone(),
            lock: None,
        });
        let rail_snap = pivot.as_ref().and_then(|pivot| RailSnap::new(pivot, &targets));
        let ends = [seam.start(), seam.end()];
        Some(Self { seam, handle, origin, pivot, edge_snap, rail_snap, ends, guide: None })
    }

    /// Smart guides of the snap the latest placement holds.
    pub(super) fn guides(&self) -> &[SnapGuide] {
        self.guide.as_slice()
    }

    /// The participants' mappings for the pointer at `point`, or `None` when the pointer has no
    /// valid placement.
    pub(super) fn resolve(
        &mut self,
        point: Point,
        scale: f64,
        suppressed: bool,
    ) -> Option<Mappings> {
        if self.handle == Handle::Middle {
            let normal = self.seam.edge.normal();
            let raw = dot(normal, sub(point, self.origin));
            let (snapped, guide) = match &mut self.edge_snap {
                Some(snap) => snap.resolve(raw, scale, suppressed),
                None => (raw, None),
            };
            self.guide = guide;
            let (distance, mappings) = translate(&self.seam, snapped, 2.0 * scale);
            let delta = self::scale(normal, distance);
            self.ends = [add(self.seam.start(), delta), add(self.seam.end(), delta)];
            return Some(mappings);
        }
        let pivot = self.pivot.as_ref()?;
        let (snapped, guide) = match &mut self.rail_snap {
            Some(snap) => snap.resolve(point, scale, suppressed),
            None => (point, None),
        };
        self.guide = guide;
        let target = pivot.constrained(snapped, SNAP * scale);
        let mappings = pivot.mappings(target).ok()?;
        self.ends = match self.handle {
            Handle::Start => [target, self.seam.end()],
            _ => [self.seam.start(), target],
        };
        Some(mappings)
    }
}

/// The participants' mappings after moving the seam `distance` along its normal, cut back to
/// the farthest valid fraction when the full move is invalid.
fn translate(seam: &Seam, distance: f64, thickness: f64) -> (f64, Mappings) {
    if let Some(mappings) = translated(seam, distance, thickness) {
        return (distance, mappings);
    }
    let (mut valid, mut invalid) = (0.0, 1.0);
    let mut mappings = translated(seam, 0.0, thickness).unwrap_or_default();
    for _ in 0..SEARCH_STEPS {
        let fraction = (valid + invalid) * 0.5;
        match translated(seam, distance * fraction, thickness) {
            Some(found) => {
                valid = fraction;
                mappings = found;
            }
            None => invalid = fraction,
        }
    }
    (distance * valid, mappings)
}

fn translated(seam: &Seam, distance: f64, thickness: f64) -> Option<Mappings> {
    if distance.abs() <= 1e-9 {
        return Some(
            seam.participants.iter().map(|item| (item.layer, item.mapping.clone())).collect(),
        );
    }
    let displacement = scale(seam.edge.normal(), distance);
    seam.participants
        .iter()
        .map(|participant| {
            let target: Vec<Point> = participant
                .scene
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    if participant.moving.contains(&index) {
                        add(*point, displacement)
                    } else {
                        *point
                    }
                })
                .collect();
            if participant.area * signed_area_twice(&target) <= 0.0 {
                return None;
            }
            let required = thickness.min(participant.clearance);
            if clearance(&target, &participant.moving)? < required - 1e-9 {
                return None;
            }
            Some((participant.layer, boundary_mapping(&participant.source, &target)?))
        })
        .collect()
}

/// One affine map when the first valid vertex triangle carries every vertex, otherwise a
/// piecewise map.
fn boundary_mapping(source: &[Point], target: &[Point]) -> Option<CoverageMapping> {
    let count = source.len();
    for index in 0..count {
        let indexes = [(index + count - 1) % count, index, (index + 1) % count];
        let Some(affine) = triangle_affine(indexes.map(|i| source[i]), indexes.map(|i| target[i]))
        else {
            continue;
        };
        let close = source.iter().zip(target).all(|(from, to)| {
            let mapped = affine.map(*from);
            let size = [mapped[0], mapped[1], to[0], to[1]]
                .iter()
                .fold(1.0_f64, |high, value| high.max(value.abs()));
            distance(mapped, *to) <= 1e-9 * size
        });
        if close {
            return Some(CoverageMapping::Affine(affine));
        }
        break;
    }
    if count == 3 {
        return None;
    }
    PiecewiseMapping::new(source, target).ok().map(CoverageMapping::Piecewise)
}

impl EdgeSnap {
    /// The snapped distance along the source's normal for the pointer's distance `raw`, with
    /// the guide of the target it snapped to.
    fn resolve(&mut self, raw: f64, scale: f64, suppressed: bool) -> (f64, Option<SnapGuide>) {
        if suppressed {
            self.lock = None;
            return (raw, None);
        }
        let moving = self.source.translated(raw);
        let offset = moving.offset();
        if let Some((owner, line)) = self.lock {
            let held = self.targets.iter().find(|target| {
                target.owner == owner
                    && (target.offset() - line).abs() <= 1e-9_f64.max(line.abs() * 1e-12)
                    && parallel(&moving, target)
                    && (target.offset() - offset).abs() <= RELEASE * scale
            });
            if let Some(target) = held {
                let resolved = raw + line - offset;
                return (resolved, Some(self.guide(resolved, target)));
            }
        }
        self.lock = None;
        let threshold = SNAP * scale;
        let best = self
            .targets
            .iter()
            .filter(|target| {
                target.owner != moving.owner
                    && parallel(&moving, target)
                    && overlap(&moving, target) >= scale
                    && (target.offset() - offset).abs() <= threshold
            })
            .min_by(|a, b| {
                let key = |edge: &Edge| (edge.offset() - offset).abs();
                key(a)
                    .total_cmp(&key(b))
                    .then(b.priority.cmp(&a.priority))
                    .then(overlap(&moving, b).total_cmp(&overlap(&moving, a)))
                    .then(a.owner.cmp(&b.owner))
                    .then(a.start[0].total_cmp(&b.start[0]))
                    .then(a.start[1].total_cmp(&b.start[1]))
            });
        match best {
            Some(target) => {
                self.lock = Some((target.owner, target.offset()));
                let resolved = raw + target.offset() - offset;
                (resolved, Some(self.guide(resolved, target)))
            }
            None => (raw, None),
        }
    }

    /// CuteCanvas's oriented guide: along the target's line, spanning both edges' extents.
    fn guide(&self, resolved: f64, target: &Edge) -> SnapGuide {
        let source = self.source.translated(resolved);
        let (tangent, normal) = (source.tangent(), source.normal());
        let (own, other) = (source.interval(), target.interval());
        let (start, end) = (own[0].min(other[0]), own[1].max(other[1]));
        let line = scale(normal, target.offset());
        SnapGuide { start: add(scale(tangent, start), line), end: add(scale(tangent, end), line) }
    }
}

/// How far two parallel edges overlap along the first one's tangent.
fn overlap(first: &Edge, second: &Edge) -> f64 {
    let tangent = first.tangent();
    let values = [dot(tangent, second.start), dot(tangent, second.end)];
    let own = first.interval();
    (own[1].min(values[0].max(values[1])) - own[0].max(values[0].min(values[1]))).max(0.0)
}

impl Pivot {
    /// The pivot of `handle`, when every participant's outer edge at the moving end shares one
    /// rail; `edges` are every visible layer's edges.
    pub(super) fn of(seam: &Seam, handle: Handle, edges: &[Edge], tolerance: f64) -> Option<Self> {
        let tolerance = tolerance.max(1e-9);
        let source = Self::source(seam, handle, tolerance)?;
        Self::at(source, handle, edges, tolerance)
    }

    /// The seam whose participants all end at the moving end: the seam itself, or the only
    /// component seam ending there.
    fn source(seam: &Seam, handle: Handle, tolerance: f64) -> Option<&Seam> {
        if seam.participants.iter().all(|item| endpoint(item, seam, handle, tolerance).is_some()) {
            return Some(seam);
        }
        let moving = if handle == Handle::Start { seam.start() } else { seam.end() };
        let mut candidates = seam.components.iter().filter(|component| {
            distance(component.start(), moving).min(distance(component.end(), moving)) <= tolerance
        });
        let only = candidates.next()?;
        candidates.next().is_none().then_some(only)
    }

    fn at(seam: &Seam, handle: Handle, edges: &[Edge], tolerance: f64) -> Option<Self> {
        let (moving, fixed) = match handle {
            Handle::Start => (seam.start(), seam.end()),
            _ => (seam.end(), seam.start()),
        };
        let other = if handle == Handle::Start { Handle::End } else { Handle::Start };
        let (mut outgoing, mut rail, mut corners) = (Vec::new(), Vec::new(), Vec::new());
        let mut collapsed = false;
        for participant in &seam.participants {
            let corner = endpoint(participant, seam, handle, tolerance)?;
            endpoint(participant, seam, other, tolerance)?;
            let exterior = exterior(participant, usize::from(handle != Handle::Start), tolerance)?;
            if distance(participant.source[corner], participant.source[exterior]) <= tolerance {
                return None;
            }
            corners.push(corner);
            let outer = participant.scene[exterior];
            if distance(outer, moving) <= tolerance {
                collapsed = true;
                rail.push(moving);
                continue;
            }
            let joined = edges.iter().any(|edge| {
                edge.owner == Owner::Layer(participant.layer)
                    && connects(edge, moving, outer, tolerance)
            });
            if !joined || parallel_vectors(sub(outer, moving), seam.edge.tangent()) {
                return None;
            }
            outgoing.push(outer);
            rail.push(outer);
        }
        let first = sub(*outgoing.first()?, moving);
        if !outgoing[1..].iter().all(|point| parallel_vectors(first, sub(*point, moving))) {
            return None;
        }
        let projections: Vec<f64> =
            rail.iter().map(|point| dot(first, sub(*point, moving))).collect();
        let low = projections.iter().copied().fold(f64::INFINITY, f64::min);
        let high = projections.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if (!collapsed && (low >= 0.0 || high <= 0.0)) || low == high {
            return None;
        }
        let position = |value: f64| projections.iter().position(|projection| *projection == value);
        let rail = [rail[position(low)?], rail[position(high)?]];
        Some(Self { fixed, rail, participants: seam.participants.clone(), corners })
    }

    /// The rail point a pointer drags to: projected and clamped, joining a rail end within `join`.
    fn constrained(&self, pointer: Point, join: f64) -> Point {
        let along = sub(self.rail[1], self.rail[0]);
        let length = distance(self.rail[0], self.rail[1]);
        let direction = scale(along, 1.0 / length);
        let join = join.max(0.0).min(length * 0.5);
        let projection = dot(sub(pointer, self.rail[0]), direction);
        if projection <= join {
            return self.rail[0];
        }
        if projection >= length - join {
            return self.rail[1];
        }
        add(self.rail[0], scale(direction, projection.clamp(0.0, length)))
    }

    /// Every participant's mapping with its moving corner at `point`.
    fn mappings(&self, point: Point) -> Result<Mappings, CanvasError> {
        self.participants
            .iter()
            .zip(&self.corners)
            .map(|(participant, corner)| {
                let mut target = participant.scene.clone();
                target[*corner] = point;
                let mapping = match joined_mapping(&participant.source, &target, *corner)? {
                    Some(mapping) => mapping,
                    None => CoverageMapping::Piecewise(PiecewiseMapping::new(
                        &participant.source,
                        &target,
                    )?),
                };
                Ok((participant.layer, mapping))
            })
            .collect()
    }

    /// The rail direction in document coordinates, for the endpoint cursor.
    pub(super) fn rail(&self) -> [Point; 2] {
        self.rail
    }
    /// The seam end that stays put.
    pub(super) const fn fixed(&self) -> Point {
        self.fixed
    }
}

/// The participant's vertex at the handle's seam end, when it lies there.
fn endpoint(
    participant: &Participant,
    seam: &Seam,
    handle: Handle,
    tolerance: f64,
) -> Option<usize> {
    let (position, expected) = match handle {
        Handle::Start => (0, seam.start()),
        _ => (1, seam.end()),
    };
    let index = participant.seam[position];
    (distance(participant.scene[index], expected) <= tolerance).then_some(index)
}

/// The boundary neighbour of the moving seam vertex away from the seam chain.
fn exterior(participant: &Participant, position: usize, tolerance: f64) -> Option<usize> {
    let (moving, fixed) = (participant.seam[position], participant.seam[1 - position]);
    let (a, b) = (participant.scene[moving], participant.scene[fixed]);
    let forward = chain_length(&participant.scene, moving, fixed, a, b, tolerance);
    let backward = chain_length(&participant.scene, fixed, moving, b, a, tolerance);
    if forward.is_some() == backward.is_some() {
        return None;
    }
    let count = participant.scene.len();
    Some(if forward.is_some() { (moving + count - 1) % count } else { (moving + 1) % count })
}

fn connects(edge: &Edge, first: Point, second: Point, tolerance: f64) -> bool {
    (distance(edge.start, first) <= tolerance && distance(edge.end, second) <= tolerance)
        || (distance(edge.end, first) <= tolerance && distance(edge.start, second) <= tolerance)
}

fn parallel_vectors(first: Point, second: Point) -> bool {
    let (a, b) = (first[0].hypot(first[1]), second[0].hypot(second[1]));
    a > 1e-12 && b > 1e-12 && cross(first, second).abs() <= 1e-4 * a * b
}
