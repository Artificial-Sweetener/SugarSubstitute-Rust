// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Snap a moving transform box to the canvas, other layers and the selection, as CuteCanvas's
//! movement snapping does.
//!
//! The box's axis-aligned bounds move; on each axis its start, centre and end are matched
//! against the same features of stationary targets: the canvas (priority 20, whose centre also
//! takes the box's edges and whose edges take its centre), the selection (15) and the content
//! bounds of every other visible layer (10). A feature within 8 device pixels snaps, and the
//! snap holds until it is more than 12 device pixels away. The closest match wins, then the
//! higher priority, then like features before start-to-end and centre-to-edge relationships.
//! Ctrl suppresses snapping.
use crate::{DocumentSnapshot, LayerContent, LayerId};

/// Device pixels within which a feature snaps.
const THRESHOLD: f64 = 8.0;
/// Further device pixels a held snap tolerates before it releases.
const RELEASE: f64 = 4.0;

/// One feature of a box along an axis.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Feature {
    Start,
    Center,
    End,
}

/// What a target belongs to, ordering ties after distance, priority and relationship.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Owner {
    Layer(u64),
    Canvas,
    Selection,
}

/// One stationary feature position on an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    position: f64,
    feature: Feature,
    priority: i32,
    owner: Owner,
    cross: bool,
}

/// Targets and the held snap of one axis.
#[derive(Debug, Default)]
struct Axis {
    targets: Vec<Target>,
    lock: Option<(Feature, Target)>,
}

/// Snapping for one move gesture of a box with `source` bounds `[left, top, right, bottom]`.
#[derive(Debug)]
pub(super) struct MoveSnap {
    source: [f64; 4],
    axes: [Axis; 2],
}

impl MoveSnap {
    /// Collect the stationary targets for a box moving `moving`'s content, or the selected
    /// pixels when `selection` is set, so the selection is not its own target.
    pub(super) fn begin(
        document: &DocumentSnapshot,
        moving: LayerId,
        selection: bool,
        source: [f64; 4],
    ) -> Self {
        let mut snap = Self { source, axes: [Axis::default(), Axis::default()] };
        let extent = document.extent();
        let canvas = [0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())];
        snap.add(canvas, 20, Owner::Canvas, true);
        if let Some(bounds) = document.selection().and_then(|selection| selection.bounds()) {
            let bounds = [bounds.x(), bounds.y(), bounds.right(), bounds.bottom()].map(f64::from);
            if !selection && bounds != source {
                snap.add(bounds, 15, Owner::Selection, false);
            }
        }
        for layer in
            document.layers().iter().filter(|layer| layer.visible() && layer.id() != moving)
        {
            let bounds = match layer.content() {
                LayerContent::Raster { .. } => Some(canvas),
                LayerContent::PlacedRaster { bounds: [left, top, width, height], .. } => {
                    Some([*left, *top, left + width, top + height])
                }
                LayerContent::VectorRectangle(rectangle) => Some(rectangle.bounds()),
                LayerContent::Coverage { coverage, .. } => {
                    coverage.content_bounds().map(|bounds| bounds.map(|value| value as f64))
                }
                LayerContent::Exposure { .. } => None,
            };
            if let Some(bounds) = bounds {
                snap.add(bounds, 10, Owner::Layer(layer.id().get()), false);
            }
        }
        snap
    }

    fn add(
        &mut self,
        [left, top, right, bottom]: [f64; 4],
        priority: i32,
        owner: Owner,
        cross: bool,
    ) {
        for (axis, [start, end]) in self.axes.iter_mut().zip([[left, right], [top, bottom]]) {
            for (feature, position) in [
                (Feature::Start, start),
                (Feature::Center, (start + end) / 2.0),
                (Feature::End, end),
            ] {
                axis.targets.push(Target { position, feature, priority, owner, cross });
            }
        }
    }

    /// The snapped move for a raw `delta` at camera `zoom`; `suppressed` releases any snap.
    pub(super) fn resolve(&mut self, delta: [f64; 2], zoom: f64, suppressed: bool) -> [f64; 2] {
        let scale = 1.0 / zoom.max(1e-9);
        let mut resolved = delta;
        for (index, axis) in self.axes.iter_mut().enumerate() {
            let [start, end] = [self.source[index], self.source[index + 2]];
            let features = [
                (Feature::Start, start + delta[index]),
                (Feature::Center, (start + end) / 2.0 + delta[index]),
                (Feature::End, end + delta[index]),
            ];
            resolved[index] = axis.resolve(delta[index], features, scale, suppressed);
        }
        resolved
    }
}

impl Axis {
    fn resolve(
        &mut self,
        raw: f64,
        features: [(Feature, f64); 3],
        scale: f64,
        suppressed: bool,
    ) -> f64 {
        if suppressed {
            self.lock = None;
            return raw;
        }
        let position = |feature: Feature| {
            features.iter().find(|(kind, _)| *kind == feature).map_or(0.0, |(_, value)| *value)
        };
        if let Some((moving, target)) = self.lock {
            let correction = target.position - position(moving);
            if correction.abs() <= (THRESHOLD + RELEASE) * scale {
                return raw + correction;
            }
        }
        let threshold = THRESHOLD * scale;
        let best = features
            .iter()
            .flat_map(|(moving, at)| self.targets.iter().map(move |target| (*moving, *at, target)))
            .filter_map(|(moving, at, target)| {
                let rank = rank(moving, target.feature, target.cross)?;
                let correction = target.position - at;
                (correction.abs() <= threshold).then_some((correction, rank, moving, *target))
            })
            .min_by(|a, b| {
                a.0.abs()
                    .total_cmp(&b.0.abs())
                    .then(b.3.priority.cmp(&a.3.priority))
                    .then(a.1.cmp(&b.1))
                    .then(a.3.position.total_cmp(&b.3.position))
                    .then(a.3.owner.cmp(&b.3.owner))
                    .then(a.2.cmp(&b.2))
            });
        match best {
            Some((correction, _, moving, target)) => {
                self.lock = Some((moving, target));
                raw + correction
            }
            None => {
                self.lock = None;
                raw
            }
        }
    }
}

/// CuteCanvas's relationship rank: like features first, then start to end, then centres to
/// edges where the target accepts them; anything else never snaps.
fn rank(moving: Feature, target: Feature, cross: bool) -> Option<u8> {
    if moving == target {
        return Some(0);
    }
    if matches!((moving, target), (Feature::Start, Feature::End) | (Feature::End, Feature::Start)) {
        return Some(1);
    }
    (cross && (moving == Feature::Center || target == Feature::Center)).then_some(2)
}
