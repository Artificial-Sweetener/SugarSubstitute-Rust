// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Snap a moving box to the canvas, other layers and the selection, as CuteCanvas's movement
//! snapping does for Move and for Transform's move handle.
//!
//! The box's axis-aligned bounds move; on each axis its start, centre and end are matched
//! against the same features of stationary targets: the canvas (priority 20), the selection (15)
//! and the content bounds of every other visible layer (10). Like features snap together, and a
//! start snaps to an end; only the canvas centre also takes the box's start and end. A feature
//! within 8 device pixels snaps, and the snap holds until it is more than 12 device pixels away.
//! Ctrl suppresses snapping.
//!
//! Ties resolve exactly as CuteCanvas's ordered match tuple does: the smallest correction, then
//! the higher priority, then like features before start-to-end and centre relationships, then
//! the lower target position, then the target owner's name and finally the moving feature's
//! name, so `center` wins over `end` and `end` over `start`. Each held snap shows a smart guide
//! along the aligned line, spanning the moved box and the target.
use crate::{DocumentSnapshot, LayerContent, LayerId, SnapGuide};

/// Device pixels within which a feature snaps.
const THRESHOLD: f64 = 8.0;
/// Further device pixels a held snap tolerates before it releases.
const RELEASE: f64 = 4.0;

/// One feature of a box along an axis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Feature {
    Start,
    Center,
    End,
}
impl Feature {
    /// CuteCanvas's tie order of moving features: their names, `center` < `end` < `start`.
    const fn name_order(self) -> u8 {
        match self {
            Self::Center => 0,
            Self::End => 1,
            Self::Start => 2,
        }
    }
}

/// What a target belongs to, ordered as CuteCanvas's owner names order ties: `composition`
/// before layer identities before `selection`. Owners of one priority are always layers, which
/// order by identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Owner {
    Canvas,
    Layer(u64),
    Selection,
}

/// One stationary feature position on an axis, with the target's extent across the axis.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    position: f64,
    feature: Feature,
    priority: i32,
    owner: Owner,
    /// Whether a moving feature of another kind may snap here: only the canvas centre.
    cross: bool,
    span: [f64; 2],
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
    guides: [SnapGuide; 2],
    shown: usize,
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
        let none = SnapGuide { start: [0.0; 2], end: [0.0; 2] };
        let mut snap =
            Self { source, axes: [Axis::default(), Axis::default()], guides: [none; 2], shown: 0 };
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
        cross_center: bool,
    ) {
        if right <= left || bottom <= top {
            return;
        }
        let axes = [([left, right], [top, bottom]), ([top, bottom], [left, right])];
        for (axis, ([start, end], span)) in self.axes.iter_mut().zip(axes) {
            for (feature, position) in [
                (Feature::Start, start),
                (Feature::Center, (start + end) / 2.0),
                (Feature::End, end),
            ] {
                let cross = cross_center && feature == Feature::Center;
                axis.targets.push(Target { position, feature, priority, owner, cross, span });
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
        self.shown = 0;
        for index in 0..2 {
            let Some((_, target)) = self.axes[index].lock else {
                continue;
            };
            // The guide spans the moved box across the snapped axis and the target.
            let across = 1 - index;
            let span = [
                (self.source[across] + resolved[across]).min(target.span[0]),
                (self.source[across + 2] + resolved[across]).max(target.span[1]),
            ];
            let point = |along: f64| {
                if index == 0 { [target.position, along] } else { [along, target.position] }
            };
            self.guides[self.shown] = SnapGuide { start: point(span[0]), end: point(span[1]) };
            self.shown += 1;
        }
        resolved
    }

    /// Smart guides of the snaps the latest move holds: a vertical line for a horizontal snap,
    /// then a horizontal line for a vertical one.
    pub(super) fn guides(&self) -> &[SnapGuide] {
        &self.guides[..self.shown]
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
                    .then(a.2.name_order().cmp(&b.2.name_order()))
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

/// CuteCanvas's relationship rank: like features first, then start to end, then a centre
/// relationship where the target accepts another feature; anything else never snaps.
fn rank(moving: Feature, target: Feature, cross: bool) -> Option<u8> {
    if moving == target {
        return Some(0);
    }
    if matches!((moving, target), (Feature::Start, Feature::End) | (Feature::End, Feature::Start)) {
        return Some(1);
    }
    (cross && (moving == Feature::Center || target == Feature::Center)).then_some(2)
}

#[cfg(test)]
mod tests;
