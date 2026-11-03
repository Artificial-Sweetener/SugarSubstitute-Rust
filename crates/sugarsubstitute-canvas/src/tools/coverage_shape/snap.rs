// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Snap drawn shape corners to document, image-layer and selection edges and centres.
//!
//! This follows CuteCanvas's default authoring policy: targets are captured once per gesture,
//! each axis snaps independently within 8 device pixels and keeps its target until the pointer
//! moves 4 pixels beyond that, the nearest target wins with ties going to the document, then the
//! selection, then image layers, and a constrained square keeps equal sides when only one axis
//! snaps.
use crate::{DocumentSnapshot, LayerContent};
use std::cmp::Ordering;

const THRESHOLD: f64 = 8.0;
const RELEASE: f64 = 4.0;
const DOCUMENT: u8 = 20;
const SELECTION: u8 = 15;
const LAYER: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    position: f64,
    priority: u8,
}

#[derive(Debug, Default)]
struct Axis {
    targets: Vec<Target>,
    lock: Option<Target>,
}
impl Axis {
    /// Snapped value and whether it snapped; `scale` is document units per device pixel.
    fn resolve(&mut self, raw: f64, scale: f64) -> (f64, bool) {
        if let Some(lock) = self.lock
            && (lock.position - raw).abs() <= (THRESHOLD + RELEASE) * scale
        {
            return (lock.position, true);
        }
        let threshold = THRESHOLD * scale;
        self.lock = self
            .targets
            .iter()
            .filter(|target| (target.position - raw).abs() <= threshold)
            .min_by(|left, right| {
                let distance = |target: &Target| (target.position - raw).abs();
                distance(left)
                    .total_cmp(&distance(right))
                    .then(right.priority.cmp(&left.priority))
                    .then(left.position.total_cmp(&right.position))
            })
            .copied();
        self.lock.map_or((raw, false), |target| (target.position, true))
    }
}

/// One gesture's snap targets, anchor and per-axis locks.
#[derive(Debug)]
pub(super) struct Snapper {
    x: Axis,
    y: Axis,
    anchor: [f64; 2],
}
impl Snapper {
    /// Capture targets and place the anchor; `suppressed` (Ctrl) leaves it where it was pressed.
    pub(super) fn begin(
        document: &DocumentSnapshot,
        anchor: [f64; 2],
        scale: f64,
        suppressed: bool,
    ) -> Self {
        let mut snapper = Self { x: Axis::default(), y: Axis::default(), anchor };
        let extent = document.extent();
        snapper.add([0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())], DOCUMENT);
        for layer in document.layers().iter().filter(|layer| layer.visible()) {
            match layer.content() {
                LayerContent::Raster { extent, .. } => snapper
                    .add([0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())], LAYER),
                LayerContent::PlacedRaster { bounds: [left, top, width, height], .. } => {
                    snapper.add([*left, *top, left + width, top + height], LAYER);
                }
                _ => {}
            }
        }
        if let Some(bounds) = document.selection().and_then(|selection| selection.bounds()) {
            snapper.add(
                [
                    f64::from(bounds.x()),
                    f64::from(bounds.y()),
                    f64::from(bounds.right()),
                    f64::from(bounds.bottom()),
                ],
                SELECTION,
            );
        }
        if !suppressed {
            let resolved =
                [snapper.x.resolve(anchor[0], scale).0, snapper.y.resolve(anchor[1], scale).0];
            snapper.anchor = resolved;
        }
        snapper.x.lock = None;
        snapper.y.lock = None;
        snapper
    }
    /// The placed anchor.
    pub(super) const fn anchor(&self) -> [f64; 2] {
        self.anchor
    }
    /// Resolve the dragged corner. Ctrl leaves it unsnapped; Shift squares it about the anchor
    /// before snapping.
    pub(super) fn update(
        &mut self,
        point: [f64; 2],
        scale: f64,
        suppressed: bool,
        constrain: bool,
    ) -> [f64; 2] {
        if suppressed {
            return point;
        }
        let proposed = if constrain { square(self.anchor, point) } else { point };
        let (x, snapped_x) = self.x.resolve(proposed[0], scale);
        let (y, snapped_y) = self.y.resolve(proposed[1], scale);
        if !constrain || !(snapped_x || snapped_y) {
            return [x, y];
        }
        let [dx, dy] = [x - self.anchor[0], y - self.anchor[1]];
        if (dx.abs() - dy.abs()).abs() <= 1e-9 {
            return [x, y];
        }
        let correction = |snapped, value: f64, raw: f64| {
            if snapped { (value - raw).abs() } else { f64::INFINITY }
        };
        let keep_x = correction(snapped_x, x, proposed[0])
            .partial_cmp(&correction(snapped_y, y, proposed[1]))
            .is_none_or(|order| order != Ordering::Greater);
        if keep_x {
            self.y.lock = None;
            [x, self.anchor[1] + direction(dy) * dx.abs()]
        } else {
            self.x.lock = None;
            [self.anchor[0] + direction(dx) * dy.abs(), y]
        }
    }
    fn add(&mut self, [left, top, right, bottom]: [f64; 4], priority: u8) {
        if !(left < right && top < bottom) {
            return;
        }
        for position in [left, (left + right) / 2.0, right] {
            self.x.targets.push(Target { position, priority });
        }
        for position in [top, (top + bottom) / 2.0, bottom] {
            self.y.targets.push(Target { position, priority });
        }
    }
}

/// The corner making a square with `anchor`, on the side the pointer is.
pub(super) fn square(anchor: [f64; 2], point: [f64; 2]) -> [f64; 2] {
    let [dx, dy] = [point[0] - anchor[0], point[1] - anchor[1]];
    let side = dx.abs().max(dy.abs());
    [anchor[0] + direction(dx) * side, anchor[1] + direction(dy) * side]
}

const fn direction(value: f64) -> f64 {
    if value < 0.0 { -1.0 } else { 1.0 }
}
