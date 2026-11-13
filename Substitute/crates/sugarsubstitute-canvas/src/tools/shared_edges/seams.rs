// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Find the edges adjacent masks share, as CuteCanvas's shared-edge discovery does.
//!
//! Every visible layer contributes its boundary's edges: a mask its convex boundary under its
//! current mapping, other layers their bounds. Two edges of different eligible layers form a
//! seam where they are parallel, lie on one line within a quarter device pixel, overlap by at
//! least 8 device pixels and have their owners' centres on opposite sides. Seams touching along
//! one line join into one seam with every participant beside it, and a seam any ineligible
//! layer's edge touches is dropped. Each participant keeps its boundary with the seam's ends
//! inserted, the matching source boundary through its mapping, the seam's vertex indexes and
//! the chain of vertices on the seam's line that moves with it.
use super::edges::{
    Edge, Owner, Point, add, centroid, distance, dot, on_line, on_segment, parallel, parameter,
    polygon_edges, scale, segment_distance, segments_distance, signed_area_twice,
};
use crate::LayerId;
use sugarsubstitute_image_ops::CoverageMapping;

/// One visible layer as discovery sees it.
#[derive(Clone, Debug)]
pub(super) struct Shape {
    pub(super) layer: LayerId,
    /// Whether its seams can be dragged: every mask while this tool is open.
    pub(super) eligible: bool,
    /// How its original coverage maps onto the document now.
    pub(super) mapping: CoverageMapping,
    /// Its boundary in document coordinates; fewer than three points takes no part.
    pub(super) boundary: Vec<Point>,
}

/// One mask beside a seam.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Participant {
    pub(super) layer: LayerId,
    pub(super) mapping: CoverageMapping,
    pub(super) source: Vec<Point>,
    pub(super) scene: Vec<Point>,
    pub(super) seam: [usize; 2],
    pub(super) moving: Vec<usize>,
    pub(super) clearance: f64,
    pub(super) area: f64,
}

/// One seam: an edge interval and every mask beside it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Seam {
    pub(super) edge: Edge,
    pub(super) interval: [f64; 2],
    pub(super) participants: Vec<Participant>,
    pub(super) components: Vec<Seam>,
}

/// Which part of a seam a press grips.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Handle {
    Start,
    Middle,
    End,
}

/// Every seam at one camera scale, with the edges pivots slide along.
#[derive(Debug, Default)]
pub(super) struct Discovery {
    pub(super) edges: Vec<Edge>,
    pub(super) seams: Vec<Seam>,
    pub(super) tolerance: f64,
}

impl Participant {
    fn new(
        layer: LayerId,
        mapping: CoverageMapping,
        source: Vec<Point>,
        scene: Vec<Point>,
        seam: [usize; 2],
        tolerance: f64,
    ) -> Option<Self> {
        if seam[0] == seam[1] || seam.iter().any(|index| *index >= scene.len()) {
            return None;
        }
        let moving = translation_indexes(&scene, seam, tolerance)?;
        let clearance = clearance(&scene, &moving)?;
        let area = signed_area_twice(&scene);
        Some(Self { layer, mapping, source, scene, seam, moving, clearance, area })
    }
}

impl Seam {
    pub(super) fn start(&self) -> Point {
        self.at(self.interval[0])
    }
    pub(super) fn end(&self) -> Point {
        self.at(self.interval[1])
    }
    fn at(&self, position: f64) -> Point {
        add(scale(self.edge.tangent(), position), scale(self.edge.normal(), self.edge.offset()))
    }
    /// Whether the whole edge may move: only axis-aligned seams.
    pub(super) fn translatable(&self) -> bool {
        let tangent = self.edge.tangent();
        tangent[0].abs() <= 1e-6 || tangent[1].abs() <= 1e-6
    }
    pub(super) fn layers(&self) -> Vec<LayerId> {
        self.participants.iter().map(|participant| participant.layer).collect()
    }
    /// Participants and endpoints rounded to 1e-9, as CuteCanvas deduplicates seams.
    fn identity(&self) -> (Vec<LayerId>, [[i64; 2]; 2]) {
        let round = |point: Point| point.map(|value| (value * 1e9).round() as i64);
        let mut ends = [round(self.start()), round(self.end())];
        ends.sort_unstable();
        let mut layers = self.layers();
        layers.sort_unstable();
        (layers, ends)
    }
}

impl Discovery {
    /// Discover seams between `shapes` at `scale` document pixels per device pixel.
    pub(super) fn new(shapes: &[Shape], scale: f64) -> Self {
        let coincidence = (0.25 * scale).max(1e-7);
        let overlap = 8.0 * scale;
        let edges: Vec<Edge> = shapes
            .iter()
            .flat_map(|shape| polygon_edges(Owner::Layer(shape.layer), &shape.boundary, 10))
            .collect();
        let shape = |owner: Owner| match owner {
            Owner::Layer(layer) => shapes.iter().find(|shape| shape.layer == layer),
            Owner::Canvas => None,
        };
        let eligible = |owner: Owner| shape(owner).is_some_and(|shape| shape.eligible);
        let mut pairs: Vec<Seam> = Vec::new();
        for first in &edges {
            let Some(first_shape) = shape(first.owner).filter(|shape| shape.eligible) else {
                continue;
            };
            for second in &edges {
                let Some(second_shape) = shape(second.owner).filter(|shape| shape.eligible) else {
                    continue;
                };
                if first_shape.boundary.len() < 3 || second_shape.boundary.len() < 3 {
                    continue;
                }
                let found = pair(first, second, [first_shape, second_shape], coincidence, overlap);
                if let Some(seam) = found
                    && !pairs.iter().any(|existing| existing.identity() == seam.identity())
                {
                    pairs.push(seam);
                }
            }
        }
        let seams = group(pairs, coincidence)
            .into_iter()
            .filter(|seam| {
                !edges.iter().any(|edge| !eligible(edge.owner) && touches(edge, seam, coincidence))
            })
            .collect();
        Self { edges, seams, tolerance: coincidence }
    }

    /// The seam under a document point: the only seam whose endpoint is within 6 device pixels,
    /// otherwise the only seam within reach at all.
    pub(super) fn seam_at(&self, point: Point, scale: f64) -> Option<usize> {
        let reach = 6.0 * scale;
        let near: Vec<usize> = (0..self.seams.len())
            .filter(|index| {
                let seam = &self.seams[*index];
                segment_distance(point, seam.start(), seam.end()) <= reach
            })
            .collect();
        let at_end: Vec<usize> = near
            .iter()
            .copied()
            .filter(|index| {
                let seam = &self.seams[*index];
                distance(point, seam.start()).min(distance(point, seam.end())) <= reach
            })
            .collect();
        match (at_end.as_slice(), near.as_slice()) {
            ([only], _) | (_, [only]) => Some(*only),
            _ => None,
        }
    }
}

/// The handle of `seam` under a document point: an endpoint within 8 device pixels, the start
/// winning ties, otherwise the middle.
pub(super) fn handle_at(seam: &Seam, point: Point, scale: f64) -> Handle {
    let (start, end) = (distance(point, seam.start()), distance(point, seam.end()));
    let (nearest, handle) = if end < start { (end, Handle::End) } else { (start, Handle::Start) };
    if nearest <= 8.0 * scale { handle } else { Handle::Middle }
}

/// The seam two edges of different layers share, if they share one.
fn pair(
    first: &Edge,
    second: &Edge,
    shapes: [&Shape; 2],
    coincidence: f64,
    minimum: f64,
) -> Option<Seam> {
    if first.owner == second.owner || !parallel(first, second) {
        return None;
    }
    let (normal, tangent, offset) = (first.normal(), first.tangent(), first.offset());
    if (offset - dot(normal, second.start)).abs() > coincidence {
        return None;
    }
    let projected = [dot(tangent, second.start), dot(tangent, second.end)];
    let own = first.interval();
    let interval =
        [own[0].max(projected[0].min(projected[1])), own[1].min(projected[0].max(projected[1]))];
    if interval[1] - interval[0] < minimum {
        return None;
    }
    let sides = [first.center, second.center].map(|center| dot(normal, center) - offset);
    if sides[0] * sides[1] >= 0.0 {
        return None;
    }
    let edge = Edge { priority: first.priority.max(second.priority), ..*first };
    let mut seam = Seam { edge, interval, participants: Vec::new(), components: Vec::new() };
    let ends = [seam.start(), seam.end()];
    for shape in shapes {
        let scene = with_points(&shape.boundary, &ends, coincidence)?;
        let source = inverse_boundary(&shape.mapping, &scene)?;
        let indexes = endpoint_indexes(&scene, ends[0], ends[1], coincidence)?;
        let mapping = shape.mapping.clone();
        seam.participants.push(Participant::new(
            shape.layer,
            mapping,
            source,
            scene,
            indexes,
            coincidence,
        )?);
    }
    seam.participants.sort_by_key(|participant| participant.layer);
    Some(seam)
}

/// Join seams touching along one line, as CuteCanvas groups them, sorted by their ends.
fn group(mut remaining: Vec<Seam>, tolerance: f64) -> Vec<Seam> {
    let mut groups = Vec::new();
    while !remaining.is_empty() {
        let mut component = vec![remaining.remove(0)];
        let mut changed = true;
        while changed {
            changed = false;
            let mut index = 0;
            while index < remaining.len() {
                let joins =
                    component.iter().any(|seam| connected(seam, &remaining[index], tolerance));
                if joins {
                    component.push(remaining.remove(index));
                    changed = true;
                } else {
                    index += 1;
                }
            }
        }
        if let Some(seam) = merge(component, tolerance) {
            groups.push(seam);
        }
    }
    groups.sort_by(|a, b| {
        let key = |seam: &Seam| {
            let (start, end) = (seam.start(), seam.end());
            [start[0], start[1], end[0], end[1]]
        };
        key(a)
            .iter()
            .zip(key(b))
            .map(|(left, right)| left.total_cmp(&right))
            .find(|order| order.is_ne())
            .unwrap_or_else(|| a.layers().cmp(&b.layers()))
    });
    groups
}

fn connected(first: &Seam, second: &Seam, tolerance: f64) -> bool {
    if !parallel(&first.edge, &second.edge)
        || (dot(first.edge.normal(), second.start()) - first.edge.offset()).abs() > tolerance
    {
        return false;
    }
    let tangent = first.edge.tangent();
    let (a, b) = (span(first, tangent), span(second, tangent));
    a[1] + tolerance >= b[0] && b[1] + tolerance >= a[0]
}

fn span(seam: &Seam, tangent: Point) -> [f64; 2] {
    let values = [dot(tangent, seam.start()), dot(tangent, seam.end())];
    [values[0].min(values[1]), values[0].max(values[1])]
}

fn merge(component: Vec<Seam>, tolerance: f64) -> Option<Seam> {
    if component.len() == 1 {
        return component.into_iter().next();
    }
    let reference = &component[0];
    let (tangent, normal, offset) =
        (reference.edge.tangent(), reference.edge.normal(), reference.edge.offset());
    let spans: Vec<[f64; 2]> = component.iter().map(|seam| span(seam, tangent)).collect();
    let interval = [
        spans.iter().map(|span| span[0]).fold(f64::INFINITY, f64::min),
        spans.iter().map(|span| span[1]).fold(f64::NEG_INFINITY, f64::max),
    ];
    let point = |position: f64| add(scale(tangent, position), scale(normal, offset));
    let edge = Edge {
        start: point(interval[0]),
        end: point(interval[1]),
        priority: component.iter().map(|seam| seam.edge.priority).max().unwrap_or(0),
        ..reference.edge
    };
    let mut layers: Vec<LayerId> = component.iter().flat_map(Seam::layers).collect();
    layers.sort_unstable();
    layers.dedup();
    let mut merged = Seam { edge, interval, participants: Vec::new(), components: Vec::new() };
    for layer in layers {
        let participant = merged_participant(layer, &component, &merged.edge, tolerance)?;
        merged.participants.push(participant);
    }
    merged.components = component;
    Some(merged)
}

fn merged_participant(
    layer: LayerId,
    component: &[Seam],
    edge: &Edge,
    tolerance: f64,
) -> Option<Participant> {
    let evidence =
        component.iter().flat_map(|seam| &seam.participants).filter(|item| item.layer == layer);
    // The first participant with the most boundary vertices, as Python's `max` keeps.
    let base = evidence.fold(None::<&Participant>, |best, item| match best {
        Some(best) if best.scene.len() >= item.scene.len() => Some(best),
        _ => Some(item),
    })?;
    let additions: Vec<Point> = component
        .iter()
        .filter(|seam| seam.participants.iter().any(|item| item.layer == layer))
        .flat_map(|seam| [seam.start(), seam.end()])
        .collect();
    let scene = with_points(&base.scene, &additions, tolerance)?;
    let source = inverse_boundary(&base.mapping, &scene)?;
    let tangent = edge.tangent();
    let mut ends = additions.clone();
    ends.sort_by(|a, b| dot(tangent, *a).total_cmp(&dot(tangent, *b)));
    let indexes = endpoint_indexes(&scene, ends[0], ends[ends.len() - 1], tolerance)?;
    let side = dot(edge.normal(), centroid(&scene)) - edge.offset();
    if side.abs() <= tolerance {
        return None;
    }
    Participant::new(layer, base.mapping.clone(), source, scene, indexes, tolerance)
}

/// Whether an edge lies on the seam's line and touches its interval.
fn touches(edge: &Edge, seam: &Seam, tolerance: f64) -> bool {
    if !parallel(edge, &seam.edge)
        || (dot(seam.edge.normal(), edge.start) - seam.edge.offset()).abs() > tolerance
    {
        return false;
    }
    let tangent = seam.edge.tangent();
    let mut edge_values = [dot(tangent, edge.start), dot(tangent, edge.end)];
    edge_values.sort_by(f64::total_cmp);
    let seam_values = span(seam, tangent);
    edge_values[1] + tolerance >= seam_values[0] && seam_values[1] + tolerance >= edge_values[0]
}

/// The boundary with `additions` inserted into the edges they lie on.
fn with_points(boundary: &[Point], additions: &[Point], tolerance: f64) -> Option<Vec<Point>> {
    let mut points = boundary.to_vec();
    if points.len() > 1 && distance(points[0], points[points.len() - 1]) <= tolerance {
        points.pop();
    }
    let mut coalesced: Vec<Point> = Vec::new();
    for point in &points {
        if coalesced.last().is_none_or(|last| distance(*last, *point) > tolerance) {
            coalesced.push(*point);
        }
    }
    if coalesced.len() > 1 && distance(coalesced[0], coalesced[coalesced.len() - 1]) <= tolerance {
        coalesced.pop();
    }
    if coalesced.len() < 3 {
        return None;
    }
    let mut result = Vec::new();
    for (index, start) in points.iter().enumerate() {
        let end = points[(index + 1) % points.len()];
        result.push(*start);
        if distance(*start, end) <= tolerance {
            continue;
        }
        let mut interior: Vec<(f64, Point)> = additions
            .iter()
            .filter(|point| {
                on_segment(**point, *start, end, tolerance)
                    && distance(**point, *start) > tolerance
                    && distance(**point, end) > tolerance
            })
            .map(|point| (parameter(*point, *start, end), *point))
            .collect();
        interior.sort_by(|a, b| a.0.total_cmp(&b.0));
        result.extend(interior.into_iter().map(|(_, point)| point));
    }
    Some(result)
}

/// The source boundary matching a document boundary through `mapping`.
fn inverse_boundary(mapping: &CoverageMapping, boundary: &[Point]) -> Option<Vec<Point>> {
    if let (Some(target), Some(source)) = (mapping.target_boundary(), mapping.source_boundary())
        && target.len() == boundary.len()
        && target
            .iter()
            .zip(boundary)
            .all(|(expected, actual)| distance(*expected, *actual) <= 1e-9)
    {
        return Some(source.to_vec());
    }
    boundary.iter().map(|point| mapping.inverse_map(*point)).collect()
}

/// The boundary vertices at a seam's ends that bound the shortest chain along it.
fn endpoint_indexes(
    boundary: &[Point],
    start: Point,
    end: Point,
    tolerance: f64,
) -> Option<[usize; 2]> {
    let near = |target: Point| -> Vec<usize> {
        (0..boundary.len())
            .filter(|index| distance(boundary[*index], target) <= tolerance)
            .collect()
    };
    let (starts, ends) = (near(start), near(end));
    let mut ranked = Vec::new();
    for first in &starts {
        for last in &ends {
            if first == last {
                continue;
            }
            let forward = chain_length(boundary, *first, *last, start, end, tolerance);
            let backward = chain_length(boundary, *last, *first, end, start, tolerance);
            if let Some(length) = forward.into_iter().chain(backward).min() {
                ranked.push((length, *first, *last));
            }
        }
    }
    let best = ranked.iter().map(|value| value.0).min()?;
    let mut winners = ranked.iter().filter(|value| value.0 == best);
    let winner = winners.next()?;
    winners.next().is_none().then_some([winner.1, winner.2])
}

/// Steps from `from` to `to` walking forward along vertices that progress along the segment.
pub(super) fn chain_length(
    boundary: &[Point],
    from: usize,
    to: usize,
    start: Point,
    end: Point,
    tolerance: f64,
) -> Option<usize> {
    let mut index = from;
    let mut previous = f64::NEG_INFINITY;
    for length in 0..boundary.len() {
        let point = boundary[index];
        if !on_segment(point, start, end, tolerance) {
            return None;
        }
        let progress = parameter(point, start, end);
        if progress + tolerance < previous {
            return None;
        }
        previous = progress;
        if index == to {
            return Some(length);
        }
        index = (index + 1) % boundary.len();
    }
    None
}

/// Every vertex on the seam's line connected to its ends, which moves with it.
fn translation_indexes(
    boundary: &[Point],
    [first, second]: [usize; 2],
    tolerance: f64,
) -> Option<Vec<usize>> {
    let count = boundary.len();
    let (start, end) = (boundary[first], boundary[second]);
    let on: Vec<bool> =
        boundary.iter().map(|point| on_line(*point, start, end, tolerance)).collect();
    let mut moving = vec![first];
    let mut pending = vec![first];
    while let Some(index) = pending.pop() {
        for candidate in [(index + count - 1) % count, (index + 1) % count] {
            if on[candidate] && !moving.contains(&candidate) {
                moving.push(candidate);
                pending.push(candidate);
            }
        }
    }
    moving.sort_unstable();
    moving.contains(&second).then_some(moving)
}

/// The narrowest gap between the moving chain and the rest of the boundary.
pub(super) fn clearance(boundary: &[Point], moving: &[usize]) -> Option<f64> {
    let mut seam: Option<(f64, [Point; 2])> = None;
    for (position, first) in moving.iter().enumerate() {
        for second in &moving[position + 1..] {
            let length = distance(boundary[*first], boundary[*second]);
            if seam.is_none_or(|(best, _)| length > best) {
                seam = Some((length, [boundary[*first], boundary[*second]]));
            }
        }
    }
    let (_, seam) = seam?;
    let mut clearance = f64::INFINITY;
    for index in 0..boundary.len() {
        let next = (index + 1) % boundary.len();
        let gap = match (moving.contains(&index), moving.contains(&next)) {
            (true, true) => continue,
            (true, false) | (false, true) => distance(boundary[index], boundary[next]),
            (false, false) => segments_distance(seam, [boundary[index], boundary[next]]),
        };
        clearance = clearance.min(gap);
    }
    clearance.is_finite().then_some(clearance)
}
