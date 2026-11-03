// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Find the document pixels whose rendering may differ between two snapshots.
//!
//! Painting a mask or a layer mask changes a few coverage tiles; redrawing only those keeps
//! strokes responsive on large documents. Any change this module cannot bound, such as a new
//! image or a reordered stack, damages the whole document. The selection is drawn as a separate
//! presentation overlay, so selection changes never damage the composed image.
use super::{DocumentSnapshot, Layer, LayerContent};
use crate::DirtyRegions;
use std::sync::Arc;
use sugarsubstitute_image_ops::Region;

/// How one layer's change affects rendering.
enum Change {
    /// Nothing visible changed, as when a hidden layer is renamed or locked.
    Invisible,
    /// Pixels changed only inside this document region.
    Within(Region),
    /// The change cannot be bounded.
    Unbounded,
}

impl DocumentSnapshot {
    /// Document-space regions that may render differently from `earlier`, clipped to the
    /// document extent. Snapshots of another document, or of another extent, differ entirely.
    pub fn damage_since(&self, earlier: &Self) -> DirtyRegions {
        let extent = self.extent();
        let mut damage = DirtyRegions::empty(extent);
        if self.identity != earlier.identity || extent != earlier.extent() {
            return DirtyRegions::full(extent);
        }
        if Arc::ptr_eq(&self.content, &earlier.content) {
            return damage;
        }
        let (current, previous) = (self.layers(), earlier.layers());
        if current.len() != previous.len()
            || current.iter().zip(previous).any(|(now, before)| now.id != before.id)
        {
            return DirtyRegions::full(extent);
        }
        for (now, before) in current.iter().zip(previous) {
            if now == before {
                continue;
            }
            match layer_change(now, before) {
                Change::Invisible => {}
                Change::Within(region) => damage.insert(region),
                Change::Unbounded => return DirtyRegions::full(extent),
            }
        }
        damage
    }
}

fn layer_change(now: &Layer, before: &Layer) -> Change {
    let rendered = |layer: &Layer| layer.visible && layer.opacity > 0.0;
    if !rendered(now) && !rendered(before) {
        return Change::Invisible;
    }
    let presentation_equal = now.visible == before.visible && now.opacity == before.opacity;
    match (&now.content, &before.content) {
        (
            LayerContent::Coverage { coverage, appearance },
            LayerContent::Coverage { coverage: earlier, appearance: earlier_appearance },
        ) => {
            let region = if presentation_equal
                && appearance == earlier_appearance
                && now.mask == before.mask
            {
                coverage.difference(earlier)
            } else {
                union(coverage.bounds(), earlier.bounds())
            };
            region.map_or(Change::Invisible, Change::Within)
        }
        (content, earlier) if content == earlier && presentation_equal => {
            match (&now.mask, &before.mask) {
                (Some(mask), Some(earlier_mask)) => {
                    mask.difference(earlier_mask).map_or(Change::Invisible, Change::Within)
                }
                (None, None) => Change::Invisible,
                _ => Change::Unbounded,
            }
        }
        _ => Change::Unbounded,
    }
}

fn union(left: Option<Region>, right: Option<Region>) -> Option<Region> {
    match (left, right) {
        (Some(left), Some(right)) => {
            let x = left.x().min(right.x());
            let y = left.y().min(right.y());
            let width = left.right().max(right.right()) - x;
            let height = left.bottom().max(right.bottom()) - y;
            Region::new(x, y, width, height).ok()
        }
        (region, None) | (None, region) => region,
    }
}
