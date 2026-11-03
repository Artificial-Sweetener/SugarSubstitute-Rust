// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tiled grayscale coverage: the single representation behind selections, mask layers and
//! layer masks.
//!
//! Values are linear 16-bit coverage, `0` empty and [`FULL_COVERAGE`] full, on the shared
//! 128-pixel tile grid. Tiles holding the coverage's default value are not stored at all, so
//! empty masks and select-alls cost nothing at any document size; tiles whose in-extent values
//! are all equal are stored as one value. Clones share tiles, and an edit copies only the tiles
//! it touches, so undo history and published snapshots stay cheap. Every coverage carries a
//! content fingerprint, which graph evaluation uses as its structural identity.
//!
//! Every read sees only the extent. Content moved beyond it is kept aside, so geometric edits
//! that bring it back restore it, as CuteCanvas keeps a moved layer's pixels.
mod affine;
mod beyond;
mod boundary;
mod codec;
mod filters;
mod geometry;
mod mapping;
mod path;
mod tiles;
mod translate;

pub use affine::CoverageAffine;
pub use codec::{coverage_from_u8, coverage_to_u8};
pub use filters::CombineMode;
pub use geometry::CoverageResampling;
pub use mapping::{BilinearMapping, CoverageMapping, PiecewiseMapping, triangle_affine};
pub use path::Path;
pub use tiles::{CoverageBuilder, CoverageTileValues};

use crate::{Extent, ImageError, Pixel, Region, TILE_EDGE, check};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};

/// Coverage of a fully covered pixel.
pub const FULL_COVERAGE: u16 = u16::MAX;
const TILE_VALUES: usize = (TILE_EDGE * TILE_EDGE) as usize;

#[derive(Clone, Debug)]
enum Slot {
    Uniform(u16),
    Stored(Arc<CoverageTile>),
}

/// One full 128×128 tile; positions outside the coverage extent always hold zero.
#[derive(Debug)]
struct CoverageTile {
    values: Vec<u16>,
    fingerprint: [u8; 32],
    _memory: MemoryClaim,
}

/// Tiles that differ from `default`, keyed by row-major tile index.
#[derive(Debug)]
struct Tiles {
    default: u16,
    slots: BTreeMap<u64, Slot>,
    fingerprint: [u8; 32],
    _memory: MemoryClaim,
}

/// Immutable-by-default tiled coverage over one document extent.
#[derive(Clone, Debug)]
pub struct Coverage {
    extent: Extent,
    columns: u32,
    tiles: Arc<Tiles>,
    beyond: Option<Arc<beyond::Beyond>>,
}

impl PartialEq for Coverage {
    fn eq(&self, other: &Self) -> bool {
        self.extent == other.extent
            && self.tiles.fingerprint == other.tiles.fingerprint
            && self.outside() == other.outside()
    }
}

impl Coverage {
    /// Uniform coverage over `extent`, without per-tile storage.
    /// # Errors
    /// Rejects insufficient admitted memory.
    pub fn new(extent: Extent, value: u16, memory: &MemoryLease) -> Result<Self, ImageError> {
        Ok(Self {
            extent,
            columns: extent.width().div_ceil(TILE_EDGE),
            tiles: Arc::new(Tiles::new(extent, value, BTreeMap::new(), memory)?),
            beyond: None,
        })
    }

    /// Document pixel extent.
    pub const fn extent(&self) -> Extent {
        self.extent
    }

    /// Content identity covering the extent and every value inside it; content beyond the
    /// extent is not drawn and does not change it.
    pub fn fingerprint(&self) -> [u8; 32] {
        self.tiles.fingerprint
    }

    /// Coverage at one pixel, or zero outside the extent.
    pub fn value(&self, x: u32, y: u32) -> u16 {
        if !self.extent.region().contains(x, y) {
            return 0;
        }
        match self.tiles.slots.get(&self.key(x / TILE_EDGE, y / TILE_EDGE)) {
            None => self.tiles.default,
            Some(Slot::Uniform(value)) => *value,
            Some(Slot::Stored(tile)) => tile.values[tile_index(x, y)],
        }
    }

    /// The single value of uniform coverage, such as an empty mask or a select-all.
    pub fn uniform(&self) -> Option<u16> {
        let tiles = u64::from(self.columns) * u64::from(self.extent.height().div_ceil(TILE_EDGE));
        if self.tiles.slots.is_empty() {
            return Some(self.tiles.default);
        }
        let mut slots = self.tiles.slots.values();
        let Some(Slot::Uniform(first)) = slots.next() else {
            return None;
        };
        (self.tiles.slots.len() as u64 == tiles
            && slots.all(|slot| matches!(slot, Slot::Uniform(value) if value == first)))
        .then_some(*first)
    }

    /// Bounds of nonzero coverage, or `None` when nothing is covered. Bounds are exact for stored
    /// tiles and span the whole extent when untouched tiles are covered.
    pub fn bounds(&self) -> Option<Region> {
        self.bounds_at_least(1)
    }

    /// Bounds of pixels holding at least `threshold`, or `None` when there are none, as
    /// CuteCanvas finds a mask's occupied area at half coverage. Exact for stored tiles; untouched
    /// tiles at or above the threshold span the whole extent, as does a zero threshold.
    pub fn bounds_at_least(&self, threshold: u16) -> Option<Region> {
        if self.tiles.default >= threshold {
            return Some(self.extent.region());
        }
        let mut bounds: Option<[u32; 4]> = None;
        let mut include = |[left, top, right, bottom]: [u32; 4]| {
            bounds = Some(match bounds {
                None => [left, top, right, bottom],
                Some([l, t, r, b]) => [l.min(left), t.min(top), r.max(right), b.max(bottom)],
            });
        };
        for (key, slot) in &self.tiles.slots {
            let region = self.tile_region(*key).ok()?;
            match slot {
                Slot::Uniform(value) if *value < threshold => {}
                Slot::Uniform(_) => {
                    include([region.x(), region.y(), region.right(), region.bottom()]);
                }
                Slot::Stored(tile) => {
                    let mut tight: Option<[u32; 4]> = None;
                    for y in region.y()..region.bottom() {
                        for x in region.x()..region.right() {
                            if tile.values[tile_index(x, y)] >= threshold {
                                tight = Some(match tight {
                                    None => [x, y, x + 1, y + 1],
                                    Some([l, t, r, b]) => {
                                        [l.min(x), t.min(y), r.max(x + 1), b.max(y + 1)]
                                    }
                                });
                            }
                        }
                    }
                    if let Some(tight) = tight {
                        include(tight);
                    }
                }
            }
        }
        let [left, top, right, bottom] = bounds?;
        Region::new(left, top, right - left, bottom - top).ok()
    }

    /// Copy row-major values of a region inside the extent.
    /// # Errors
    /// Rejects a region outside the extent or an output of the wrong length.
    pub fn read(&self, region: Region, output: &mut [u16]) -> Result<(), ImageError> {
        self.validate(region, output.len())?;
        for part in region.tiles() {
            let slot = self.tiles.slots.get(&self.key(part.x() / TILE_EDGE, part.y() / TILE_EDGE));
            for y in part.y()..part.bottom() {
                let start = (y - region.y()) as usize * region.width() as usize
                    + (part.x() - region.x()) as usize;
                let row = &mut output[start..start + part.width() as usize];
                match slot {
                    None => row.fill(self.tiles.default),
                    Some(Slot::Uniform(value)) => row.fill(*value),
                    Some(Slot::Stored(tile)) => {
                        let source = tile_index(part.x(), y);
                        row.copy_from_slice(&tile.values[source..source + part.width() as usize]);
                    }
                }
            }
        }
        Ok(())
    }

    /// Evaluate a region as premultiplied white whose alpha is the coverage, the form graph masks
    /// and tinted overlays consume.
    /// # Errors
    /// Rejects a region outside the extent or an output of the wrong length.
    pub fn read_pixels(&self, region: Region, output: &mut [Pixel]) -> Result<(), ImageError> {
        self.validate(region, output.len())?;
        for part in region.tiles() {
            let slot = self.tiles.slots.get(&self.key(part.x() / TILE_EDGE, part.y() / TILE_EDGE));
            for y in part.y()..part.bottom() {
                let start = (y - region.y()) as usize * region.width() as usize
                    + (part.x() - region.x()) as usize;
                for (offset, pixel) in
                    output[start..start + part.width() as usize].iter_mut().enumerate()
                {
                    let value = match slot {
                        None => self.tiles.default,
                        Some(Slot::Uniform(value)) => *value,
                        Some(Slot::Stored(tile)) => tile.values[tile_index(part.x(), y) + offset],
                    };
                    let alpha = f32::from(value) / f32::from(FULL_COVERAGE);
                    *pixel = Pixel([alpha; 4]);
                }
            }
        }
        Ok(())
    }

    /// Evaluate nearest samples as [`Self::read_pixels`] does, for sparse previews.
    /// # Errors
    /// Rejects samples planned for another extent or an output of the wrong length.
    pub fn read_samples(
        &self,
        samples: &crate::RasterSamples,
        output: &mut [Pixel],
    ) -> Result<(), ImageError> {
        if samples.source() != self.extent || samples.region().pixels() != output.len() as u64 {
            return Err(ImageError::Invalid("Coverage samples differ from their request"));
        }
        let columns = samples.columns();
        for (row, y) in samples.rows().iter().enumerate() {
            for (column, x) in columns.iter().enumerate() {
                let alpha = f32::from(self.value(*x, *y)) / f32::from(FULL_COVERAGE);
                output[row * columns.len() + column] = Pixel([alpha; 4]);
            }
        }
        Ok(())
    }

    /// Replace a region with row-major values.
    /// # Errors
    /// Rejects a region outside the extent, a wrong input length or memory pressure; a failed
    /// write leaves this coverage unchanged.
    pub fn write(
        &mut self,
        region: Region,
        values: &[u16],
        memory: &MemoryLease,
    ) -> Result<(), ImageError> {
        self.validate(region, values.len())?;
        let width = region.width() as usize;
        self.rewrite(region, memory, None, |x, y, row| {
            let start = (y - region.y()) as usize * width + (x - region.x()) as usize;
            row.copy_from_slice(&values[start..start + row.len()]);
        })
    }

    /// Rewrite every value in a region through `change(x, y, current)`.
    /// # Errors
    /// Rejects a region outside the extent or memory pressure; a failed update leaves this
    /// coverage unchanged.
    pub fn update(
        &mut self,
        region: Region,
        memory: &MemoryLease,
        change: impl FnMut(u32, u32, u16) -> u16,
    ) -> Result<(), ImageError> {
        self.rewrite(region, memory, None, per_value(change))
    }

    /// [`Self::update`] inside a job, checking cancellation and deadline between tiles.
    pub(crate) fn update_in_job(
        &mut self,
        region: Region,
        context: &JobContext,
        change: impl FnMut(u32, u32, u16) -> u16,
    ) -> Result<(), ImageError> {
        self.rewrite(region, context.memory(), Some(context), per_value(change))
    }

    /// Invert every value inside the extent; content beyond it is kept as it is. Untouched tiles
    /// stay untouched, so inversion costs only the stored tiles and inverting twice restores the
    /// original identity.
    /// # Errors
    /// Preserves memory pressure.
    pub fn invert(&self, memory: &MemoryLease) -> Result<Self, ImageError> {
        let mut slots = BTreeMap::new();
        for (key, slot) in &self.tiles.slots {
            let inverted = match slot {
                Slot::Uniform(value) => Slot::Uniform(FULL_COVERAGE - value),
                Slot::Stored(tile) => {
                    let region = self.tile_region(*key)?;
                    let mut values = Vec::new();
                    values.try_reserve_exact(TILE_VALUES)?;
                    values.resize(TILE_VALUES, 0);
                    for y in region.y()..region.bottom() {
                        let start = tile_index(region.x(), y);
                        let row = start..start + region.width() as usize;
                        for (target, value) in values[row.clone()].iter_mut().zip(&tile.values[row])
                        {
                            *target = FULL_COVERAGE - value;
                        }
                    }
                    Slot::Stored(Arc::new(CoverageTile::new(values, memory)?))
                }
            };
            slots.insert(*key, inverted);
        }
        let default = FULL_COVERAGE - self.tiles.default;
        let tiles = Arc::new(Tiles::new(self.extent, default, slots, memory)?);
        Ok(Self { tiles, ..self.clone() })
    }

    /// Rewrite a region tile by tile; `change(x, y, row)` rewrites the in-region values of one
    /// tile row starting at `(x, y)`.
    fn rewrite(
        &mut self,
        region: Region,
        memory: &MemoryLease,
        context: Option<&JobContext>,
        mut change: impl FnMut(u32, u32, &mut [u16]),
    ) -> Result<(), ImageError> {
        self.validate(region, region.pixels() as usize)?;
        let default = self.tiles.default;
        let mut slots = self.tiles.slots.clone();
        for part in region.tiles() {
            if let Some(context) = context {
                check(context)?;
            }
            let key = self.key(part.x() / TILE_EDGE, part.y() / TILE_EDGE);
            let tile_region = self.tile_region(key)?;
            let mut values = Vec::new();
            values.try_reserve_exact(TILE_VALUES)?;
            match slots.get(&key) {
                Some(Slot::Stored(tile)) => values.extend_from_slice(&tile.values),
                slot => {
                    let value = match slot {
                        Some(Slot::Uniform(value)) => *value,
                        _ => default,
                    };
                    values.resize(TILE_VALUES, 0);
                    for y in tile_region.y()..tile_region.bottom() {
                        let start = tile_index(tile_region.x(), y);
                        values[start..start + tile_region.width() as usize].fill(value);
                    }
                }
            }
            for y in part.y()..part.bottom() {
                let start = tile_index(part.x(), y);
                change(part.x(), y, &mut values[start..start + part.width() as usize]);
            }
            match uniform_value(&values, tile_region) {
                Some(value) if value == default => {
                    slots.remove(&key);
                }
                Some(value) => {
                    slots.insert(key, Slot::Uniform(value));
                }
                None => {
                    slots.insert(key, Slot::Stored(Arc::new(CoverageTile::new(values, memory)?)));
                }
            }
        }
        self.tiles = Arc::new(Tiles::new(self.extent, default, slots, memory)?);
        Ok(())
    }

    /// Bounds of every tile whose values may differ from `other`, or `None` when both hold the
    /// same values. Tiles shared between the two by copy-on-write are skipped without being
    /// read, so the cost follows the edited tiles.
    pub fn difference(&self, other: &Self) -> Option<Region> {
        if self.extent != other.extent || self.tiles.default != other.tiles.default {
            return Some(self.extent.region());
        }
        if self.tiles.fingerprint == other.tiles.fingerprint {
            return None;
        }
        let mut bounds: Option<[u32; 4]> = None;
        let keys = self.tiles.slots.keys().chain(other.tiles.slots.keys());
        for key in keys {
            let same = match (self.tiles.slots.get(key), other.tiles.slots.get(key)) {
                (Some(Slot::Uniform(left)), Some(Slot::Uniform(right))) => left == right,
                (Some(Slot::Stored(left)), Some(Slot::Stored(right))) => {
                    Arc::ptr_eq(left, right) || left.fingerprint == right.fingerprint
                }
                _ => false,
            };
            if same {
                continue;
            }
            let Ok(region) = self.tile_region(*key) else {
                return Some(self.extent.region());
            };
            let [left, top, right, bottom] =
                [region.x(), region.y(), region.right(), region.bottom()];
            bounds = Some(match bounds {
                None => [left, top, right, bottom],
                Some([l, t, r, b]) => [l.min(left), t.min(top), r.max(right), b.max(bottom)],
            });
        }
        let [left, top, right, bottom] = bounds?;
        Region::new(left, top, right - left, bottom - top).ok()
    }

    /// Bytes of tile storage this coverage holds, shared storage included.
    pub fn stored_bytes(&self) -> u64 {
        let stored = self.tiles.slots.values().filter(|slot| matches!(slot, Slot::Stored(_)));
        let outside = self.outside().map_or(0, |(frame, _)| frame.stored_bytes());
        stored.count() as u64 * (TILE_VALUES * size_of::<u16>()) as u64 + outside
    }

    fn validate(&self, region: Region, length: usize) -> Result<(), ImageError> {
        if region.intersection(self.extent.region()) != Some(region) {
            return Err(ImageError::Invalid("Coverage region outside its extent"));
        }
        if region.pixels() != length as u64 {
            return Err(ImageError::Invalid("Coverage buffer length differs from its region"));
        }
        Ok(())
    }

    fn key(&self, column: u32, row: u32) -> u64 {
        u64::from(row) * u64::from(self.columns) + u64::from(column)
    }

    fn tile_region(&self, key: u64) -> Result<Region, ImageError> {
        let columns = u64::from(self.columns);
        self.extent.tile_region((key % columns) as u32, (key / columns) as u32)
    }
}

/// Adapt a per-value change to the row-wise rewrite.
fn per_value(mut change: impl FnMut(u32, u32, u16) -> u16) -> impl FnMut(u32, u32, &mut [u16]) {
    move |x, y, row| {
        for (value, column) in row.iter_mut().zip(x..) {
            *value = change(column, y, *value);
        }
    }
}

/// The single in-extent value of a tile, when all its values are equal.
fn uniform_value(values: &[u16], region: Region) -> Option<u16> {
    let first = values[tile_index(region.x(), region.y())];
    (region.y()..region.bottom())
        .all(|y| {
            let start = tile_index(region.x(), y);
            values[start..start + region.width() as usize].iter().all(|value| *value == first)
        })
        .then_some(first)
}

impl CoverageTile {
    fn new(values: Vec<u16>, memory: &MemoryLease) -> Result<Self, ImageError> {
        let claim = memory.claim((TILE_VALUES * size_of::<u16>() + 64) as u64)?;
        // Hash little-endian bytes in contiguous blocks: the digest is the same as hashing each
        // value in turn, without a hasher call per value.
        let mut hash = Sha256::new();
        let mut block = [0_u8; 2048];
        for chunk in values.chunks(block.len() / 2) {
            for (bytes, value) in block.chunks_exact_mut(2).zip(chunk) {
                bytes.copy_from_slice(&value.to_le_bytes());
            }
            hash.update(&block[..chunk.len() * 2]);
        }
        Ok(Self { values, fingerprint: hash.finalize().into(), _memory: claim })
    }
}

impl Tiles {
    fn new(
        extent: Extent,
        default: u16,
        slots: BTreeMap<u64, Slot>,
        memory: &MemoryLease,
    ) -> Result<Self, ImageError> {
        let claim = memory.claim((slots.len() * (size_of::<Slot>() + 32) + 64) as u64)?;
        let mut hash = Sha256::new();
        hash.update(b"sugarsubstitute-coverage-v1\0");
        hash.update(extent.width().to_le_bytes());
        hash.update(extent.height().to_le_bytes());
        hash.update(default.to_le_bytes());
        for (key, slot) in &slots {
            hash.update(key.to_le_bytes());
            match slot {
                Slot::Uniform(value) => {
                    hash.update([0]);
                    hash.update(value.to_le_bytes());
                }
                Slot::Stored(tile) => {
                    hash.update([1]);
                    hash.update(tile.fingerprint);
                }
            }
        }
        Ok(Self { default, slots, fingerprint: hash.finalize().into(), _memory: claim })
    }
}

/// Index of an image pixel inside its 128×128 tile.
const fn tile_index(x: u32, y: u32) -> usize {
    (y % TILE_EDGE) as usize * TILE_EDGE as usize + (x % TILE_EDGE) as usize
}

/// Multiply two coverages with nearest rounding.
#[inline]
pub(crate) const fn multiply(left: u16, right: u16) -> u16 {
    ((left as u32 * right as u32 + FULL_COVERAGE as u32 / 2) / FULL_COVERAGE as u32) as u16
}
