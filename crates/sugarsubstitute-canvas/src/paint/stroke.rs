// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Composite one stroke's dabs into coverage with CuteCanvas's mask-painting rules.
//!
//! Each dab composites over the stroke so far: painting is source-over of the dab's coverage,
//! erasing is destination-out. A dab covers the pixels whose centres lie strictly inside its
//! circle; its coverage is the opacity, held at full strength out to the hardness fraction of
//! the radius and falling linearly to nothing at the rim. Overlapping dabs of one stroke
//! therefore build up toward full coverage. A selection constrains the finished stroke once:
//! the result is the stroke-start coverage blended toward the painted coverage by the selection.
//!
//! Painting happens in dense stroke-local tiles, so a dab costs only the pixels it covers.
//! [`StrokeRaster::flush`] publishes the tiles changed since the previous flush into the result
//! coverage, which shares every untouched tile with the stroke-start coverage.

use super::dabs::Dab;
use crate::CanvasError;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::{
    CombineMode, Coverage, FULL_COVERAGE, ImageError, Region, TILE_EDGE,
};

/// How one dab joins the stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DabStyle {
    /// Fraction of the radius held at full opacity, in `[0, 1]`.
    pub(crate) hardness: f64,
    /// Peak dab coverage in `[0, 1]`.
    pub(crate) opacity: f64,
    /// Whether the dab removes coverage.
    pub(crate) erase: bool,
}

/// Stroke-local values of one coverage tile, row-major over the tile's in-extent region.
struct PaintedTile {
    region: Region,
    values: Vec<u16>,
    _claim: MemoryClaim,
}

/// One stroke's working coverage against its stroke-start coverage and optional selection.
pub(crate) struct StrokeRaster {
    base: Coverage,
    selection: Option<Coverage>,
    result: Coverage,
    painted: BTreeMap<(u32, u32), PaintedTile>,
    dirty: BTreeSet<(u32, u32)>,
    memory: MemoryLease,
}

impl StrokeRaster {
    /// Start a stroke on `base`, constrained by `selection` when one is present. Working tiles
    /// are charged to `memory` as the stroke reaches them.
    /// # Errors
    /// Rejects a selection whose extent differs from the coverage's.
    pub(crate) fn new(
        base: Coverage,
        selection: Option<Coverage>,
        memory: &MemoryLease,
    ) -> Result<Self, CanvasError> {
        if selection.as_ref().is_some_and(|selection| selection.extent() != base.extent()) {
            return Err(CanvasError::Invalid("Selection extent differs from the painted mask"));
        }
        Ok(Self {
            result: base.clone(),
            base,
            selection,
            painted: BTreeMap::new(),
            dirty: BTreeSet::new(),
            memory: memory.clone(),
        })
    }

    /// Composite one dab. Pixels outside the coverage extent are skipped.
    /// # Errors
    /// Rejects nonfinite geometry and preserves memory admission failures; a failed dab may have
    /// painted part of its footprint, and the stroke remains usable.
    pub(crate) fn stamp(&mut self, dab: Dab, style: DabStyle) -> Result<(), CanvasError> {
        let [cx, cy] = dab.center;
        if !(cx.is_finite() && cy.is_finite() && dab.diameter.is_finite()) {
            return Err(CanvasError::Invalid("Brush dab geometry is not finite"));
        }
        let extent = self.base.extent();
        let radius = (dab.diameter / 2.0).max(0.5);
        let top = ((cy - radius - 0.5).floor() + 1.0).max(0.0);
        let bottom = (cy + radius - 0.5).ceil().min(f64::from(extent.height()));
        let mode = if style.erase { CombineMode::Subtract } else { CombineMode::Add };
        let falloff = Falloff::new(style, radius);
        let mut y = top;
        while y < bottom {
            let row = y as u32;
            y += 1.0;
            let dy = f64::from(row) + 0.5 - cy;
            let remaining = radius * radius - dy * dy;
            if remaining <= 0.0 {
                continue;
            }
            let half = remaining.sqrt();
            let left = ((cx - half - 0.5).floor() + 1.0).max(0.0);
            let right = (cx + half - 0.5).ceil().min(f64::from(extent.width()));
            if left >= right {
                continue;
            }
            let (mut x, right) = (left as u32, right as u32);
            while x < right {
                let key = (row / TILE_EDGE, x / TILE_EDGE);
                let end = right.min((key.1 + 1) * TILE_EDGE);
                let tile = tile(&mut self.painted, &self.base, &self.memory, key)?;
                let start = (row - tile.region.y()) as usize * tile.region.width() as usize;
                let span = &mut tile.values[start + (x - tile.region.x()) as usize
                    ..start + (end - tile.region.x()) as usize];
                match falloff.solid() {
                    // An opaque hard dab replaces the span outright.
                    Some(value) => span.fill(if style.erase { 0 } else { value }),
                    None => {
                        for (value, column) in span.iter_mut().zip(x..end) {
                            let incoming = falloff.coverage(f64::from(column) + 0.5 - cx, dy);
                            if incoming != 0 {
                                *value = mode.apply(*value, incoming);
                            }
                        }
                    }
                }
                self.dirty.insert(key);
                x = end;
            }
        }
        Ok(())
    }

    /// Publish every tile changed since the last flush and return the stroke's coverage.
    /// # Errors
    /// Preserves allocation and memory admission failures; unpublished tiles stay pending.
    pub(crate) fn flush(&mut self) -> Result<&Coverage, CanvasError> {
        while let Some(&(row, first)) = self.dirty.first() {
            let mut last = first;
            while self.dirty.contains(&(row, last + 1)) {
                last += 1;
            }
            self.publish(row, first, last)?;
            for column in first..=last {
                self.dirty.remove(&(row, column));
            }
        }
        Ok(&self.result)
    }

    /// Write one horizontal run of painted tiles into the result, through the selection.
    fn publish(&mut self, row: u32, first: u32, last: u32) -> Result<(), CanvasError> {
        let tiles: Vec<&PaintedTile> = (first..=last)
            .map(|column| {
                self.painted
                    .get(&(row, column))
                    .ok_or(CanvasError::Invalid("Dirty brush tile was never painted"))
            })
            .collect::<Result<_, _>>()?;
        let (origin, height) = (tiles[0].region, tiles[0].region.height());
        let width = tiles.iter().map(|tile| tile.region.width()).sum::<u32>();
        let region = Region::new(origin.x(), origin.y(), width, height)?;
        let mut values = dense(region)?;
        let mut offset = 0;
        for tile in &tiles {
            let tile_width = tile.region.width() as usize;
            for (line, source) in tile.values.chunks_exact(tile_width).enumerate() {
                let start = line * width as usize + offset;
                values[start..start + tile_width].copy_from_slice(source);
            }
            offset += tile_width;
        }
        if let Some(selection) = &self.selection {
            let mut before = dense(region)?;
            let mut constraint = dense(region)?;
            self.base.read(region, &mut before)?;
            selection.read(region, &mut constraint)?;
            for ((value, before), constraint) in values.iter_mut().zip(before).zip(constraint) {
                *value = blend(before, *value, constraint);
            }
        }
        self.result.write(region, &values, &self.memory)?;
        Ok(())
    }
}

/// Dab coverage at an offset from its centre.
struct Falloff {
    opacity: f64,
    hardness: f64,
    radius: f64,
}

impl Falloff {
    fn new(style: DabStyle, radius: f64) -> Self {
        Self {
            opacity: style.opacity.clamp(0.0, 1.0),
            hardness: style.hardness.clamp(0.0, 1.0),
            radius,
        }
    }

    /// Full coverage when every covered pixel takes the same full value, as for an opaque hard
    /// dab, so a span can be filled without compositing.
    fn solid(&self) -> Option<u16> {
        (self.hardness >= 1.0 && self.opacity >= 1.0).then_some(FULL_COVERAGE)
    }

    /// Coverage of the pixel whose centre is `(dx, dy)` from the dab centre; the caller has
    /// already established that the centre lies inside the circle.
    fn coverage(&self, dx: f64, dy: f64) -> u16 {
        let strength = if self.hardness >= 1.0 {
            1.0
        } else {
            let distance = (dx * dx + dy * dy).sqrt() / self.radius;
            if distance <= self.hardness {
                1.0
            } else {
                ((1.0 - distance) / (1.0 - self.hardness)).clamp(0.0, 1.0)
            }
        };
        // Nonnegative, so adding a half and truncating rounds to nearest without a libm call.
        (self.opacity * strength * f64::from(FULL_COVERAGE) + 0.5) as u16
    }
}

/// Stroke-start coverage blended toward painted coverage by the selection coverage.
fn blend(before: u16, painted: u16, selection: u16) -> u16 {
    let full = u32::from(FULL_COVERAGE);
    let selection = u32::from(selection);
    ((u32::from(before) * (full - selection) + u32::from(painted) * selection + full / 2) / full)
        as u16
}

fn tile<'a>(
    painted: &'a mut BTreeMap<(u32, u32), PaintedTile>,
    base: &Coverage,
    memory: &MemoryLease,
    key: (u32, u32),
) -> Result<&'a mut PaintedTile, CanvasError> {
    Ok(match painted.entry(key) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            let region = base.extent().tile_region(key.1, key.0)?;
            let claim = memory
                .claim(region.pixels() * size_of::<u16>() as u64 + 64)
                .map_err(ImageError::from)?;
            let mut values = dense(region)?;
            base.read(region, &mut values)?;
            entry.insert(PaintedTile { region, values, _claim: claim })
        }
    })
}

fn dense(region: Region) -> Result<Vec<u16>, CanvasError> {
    let mut values = Vec::new();
    values.try_reserve_exact(region.pixels() as usize).map_err(ImageError::from)?;
    values.resize(region.pixels() as usize, 0);
    Ok(values)
}
