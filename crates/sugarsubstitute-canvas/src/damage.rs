// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact retained-pixel translation and disjoint exposed-edge repair.
use crate::Viewport;
use sugarsubstitute_image_ops::{Extent, Region};

/// Copy retained device pixels to a different overlapping region of the same surface.
/// A presenter must handle overlap in both axes and preserve source pixels until copied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelBlit {
    /// Source device-pixel region in the preceding presented frame.
    pub source: Region,
    /// Destination top-left device-pixel coordinates.
    pub destination: [u32; 2],
}

/// Bounded surface damage, expressed in physical device pixels and clipped to its owning extent.
/// A bounded inventory overflow conservatively collapses to the full viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirtyRegions {
    surface: Extent,
    regions: [Option<Region>; 32],
    length: usize,
}
impl DirtyRegions {
    /// An unchanged device surface.
    pub const fn empty(surface: Extent) -> Self {
        Self { surface, regions: [None; 32], length: 0 }
    }
    /// A complete drawable invalidation.
    pub fn full(surface: Extent) -> Self {
        let mut result = Self::empty(surface);
        result.regions[0] = Some(surface.region());
        result.length = 1;
        result
    }
    /// Add clipped damage. Off-surface requests have no effect.
    /// Overlapping requests remain independently valid; callers may repaint their overlap.
    pub fn insert(&mut self, region: Region) {
        let Some(region) = region.intersection(self.surface.region()) else {
            return;
        };
        if self.length == self.regions.len() {
            *self = Self::full(self.surface);
            return;
        }
        if self.regions[..self.length]
            .iter()
            .flatten()
            .any(|old| old.intersection(region) == Some(region))
        {
            return;
        }
        self.regions[self.length] = Some(region);
        self.length += 1;
    }
    /// Borrow clipped dirty rectangles without proportional or iterator-side allocation.
    pub fn regions(&self) -> impl Iterator<Item = Region> + '_ {
        self.regions[..self.length].iter().flatten().copied()
    }
    /// Whether any image pixels require preparation.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
    /// Sum of requested dirty pixels; independently inserted overlaps count separately.
    pub fn pixels(&self) -> u64 {
        self.regions().map(Region::pixels).sum()
    }
    /// Owning physical viewport dimensions.
    pub const fn surface(&self) -> Extent {
        self.surface
    }
}

/// Retained-pixel reuse followed by preparation of dirty regions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DamagePlan {
    blit: Option<PixelBlit>,
    dirty: DirtyRegions,
}
impl DamagePlan {
    /// Invalidate all pixels when content, scale, dimensions or sample phase changed.
    pub fn full(surface: Extent) -> Self {
        Self { blit: None, dirty: DirtyRegions::full(surface) }
    }
    /// Keep existing pixels; used for unchanged content with no camera movement.
    pub fn unchanged(surface: Extent) -> Self {
        Self { blit: None, dirty: DirtyRegions::empty(surface) }
    }
    /// Reuse the exact overlap of an integer translation, repairing disjoint exposed strips.
    /// Large shifts invalidate the full surface; no saturating/truncated coordinates are copied.
    /// # Errors
    /// Preserves checked region-construction failures before exposing a partial plan.
    pub fn pan(surface: Extent, dx: i32, dy: i32) -> Result<Self, crate::CanvasError> {
        if dx == 0 && dy == 0 {
            return Ok(Self::unchanged(surface));
        }
        if dx.unsigned_abs() >= surface.width() || dy.unsigned_abs() >= surface.height() {
            return Ok(Self::full(surface));
        }
        let width = surface.width() - dx.unsigned_abs();
        let height = surface.height() - dy.unsigned_abs();
        let destination = [dx.max(0) as u32, dy.max(0) as u32];
        let source_x = dx.saturating_neg().max(0) as u32;
        let source_y = dy.saturating_neg().max(0) as u32;
        let mut dirty = DirtyRegions::empty(surface);
        if dy != 0 {
            let top = if dy > 0 { 0 } else { surface.height() - dy.unsigned_abs() };
            dirty.insert(Region::new(0, top, surface.width(), dy.unsigned_abs())?);
        }
        if dx != 0 {
            let left = if dx > 0 { 0 } else { surface.width() - dx.unsigned_abs() };
            dirty.insert(Region::new(left, destination[1], dx.unsigned_abs(), height)?);
        }
        let source = Region::new(source_x, source_y, width, height)?;
        Ok(Self { blit: Some(PixelBlit { source, destination }), dirty })
    }
    /// Also repaint the device pixels that changed document regions reach on `view`'s surface.
    /// Each region's mapped bounds grow by the reconstruction filter's reach (three source
    /// pixels at magnification, a few device pixels through minifying pyramids), so resampled
    /// edges around an edit update too.
    /// # Errors
    /// Preserves viewport mapping and region construction failures.
    pub fn include_document_damage(
        &mut self,
        damage: &DirtyRegions,
        view: Viewport,
    ) -> Result<(), crate::CanvasError> {
        let surface = view.surface();
        let margin = (3.0 * view.zoom().max(1.0)).ceil() + 8.0;
        for region in damage.regions() {
            let start = view.device_at([f64::from(region.x()), f64::from(region.y())])?;
            let end = view.device_at([f64::from(region.right()), f64::from(region.bottom())])?;
            let clamp = |value: f64, limit: u32| value.clamp(0.0, f64::from(limit)) as u32;
            let left = clamp((start[0].min(end[0]) - margin).floor(), surface.width());
            let top = clamp((start[1].min(end[1]) - margin).floor(), surface.height());
            let right = clamp((start[0].max(end[0]) + margin).ceil(), surface.width());
            let bottom = clamp((start[1].max(end[1]) + margin).ceil(), surface.height());
            if left < right && top < bottom {
                self.dirty.insert(Region::new(left, top, right - left, bottom - top)?);
            }
        }
        Ok(())
    }
    /// Optional overlap copy, performed before applying prepared dirty pixels.
    pub const fn blit(&self) -> Option<PixelBlit> {
        self.blit
    }
    /// New pixel regions requiring actual graph evaluation.
    pub const fn dirty(&self) -> &DirtyRegions {
        &self.dirty
    }
}
