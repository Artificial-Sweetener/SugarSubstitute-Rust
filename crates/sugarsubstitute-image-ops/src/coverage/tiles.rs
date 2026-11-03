// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange coverage exactly, one tile at a time, for lossless persistence.
//!
//! Export visits only the tiles that differ from the coverage's default value, in ascending
//! row-major tile index order, exactly as they are held. Import accepts only that canonical
//! form, so a rebuilt coverage has the same values, storage and fingerprint as the original,
//! and malformed input is rejected rather than normalized.
use super::{Coverage, CoverageTile, Slot, TILE_VALUES, Tiles, tile_index, uniform_value};
use crate::{Extent, ImageError, TILE_EDGE};
use std::{collections::BTreeMap, sync::Arc};
use sugarsubstitute_contracts::execution::MemoryLease;

/// Values of one tile that differs from its coverage's default value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageTileValues<'a> {
    /// Every position of the tile inside the coverage extent holds this value.
    Uniform(u16),
    /// Row-major 128×128 values whose in-extent positions are not all equal; positions outside
    /// the coverage extent hold zero.
    Stored(&'a [u16]),
}

impl Coverage {
    /// Value of every tile that [`Self::tile_values`] does not visit.
    pub fn default_value(&self) -> u16 {
        self.tiles.default
    }

    /// Number of tiles on the 128-pixel grid covering the extent.
    pub fn tile_count(&self) -> u64 {
        tile_count(self.extent)
    }

    /// Tiles that differ from [`Self::default_value`], by ascending row-major tile index.
    pub fn tile_values(&self) -> impl Iterator<Item = (u64, CoverageTileValues<'_>)> + '_ {
        self.tiles.slots.iter().map(|(index, slot)| {
            let values = match slot {
                Slot::Uniform(value) => CoverageTileValues::Uniform(*value),
                Slot::Stored(tile) => CoverageTileValues::Stored(&tile.values),
            };
            (*index, values)
        })
    }
}

/// Rebuild a coverage from tiles in the canonical form [`Coverage::tile_values`] exports.
///
/// Tiles arrive by strictly ascending index. Each stored tile is charged to the supplied
/// memory lease as it is accepted; dropping the builder releases them.
#[derive(Debug)]
pub struct CoverageBuilder {
    extent: Extent,
    default: u16,
    tiles: u64,
    next: u64,
    slots: BTreeMap<u64, Slot>,
}

impl CoverageBuilder {
    /// Begin coverage over `extent` whose unvisited tiles hold `default`.
    pub fn new(extent: Extent, default: u16) -> Self {
        Self { extent, default, tiles: tile_count(extent), next: 0, slots: BTreeMap::new() }
    }

    /// Number of 16-bit values every stored tile carries.
    pub const fn stored_tile_values() -> usize {
        TILE_VALUES
    }

    /// Accept a tile whose in-extent positions all hold `value`.
    /// # Errors
    /// Rejects an index out of order or outside the grid, and a value equal to the default.
    pub fn uniform(&mut self, index: u64, value: u16) -> Result<(), ImageError> {
        self.admit(index)?;
        if value == self.default {
            return Err(ImageError::Invalid("Uniform coverage tile repeats the default value"));
        }
        self.slots.insert(index, Slot::Uniform(value));
        Ok(())
    }

    /// Accept a tile of row-major 128×128 values.
    /// # Errors
    /// Rejects an index out of order or outside the grid, a wrong value count, nonzero values
    /// outside the extent, in-extent values that are all equal, and memory pressure.
    pub fn stored(
        &mut self,
        index: u64,
        values: Vec<u16>,
        memory: &MemoryLease,
    ) -> Result<(), ImageError> {
        self.admit(index)?;
        if values.len() != TILE_VALUES {
            return Err(ImageError::Invalid("Stored coverage tile has the wrong value count"));
        }
        let columns = u64::from(self.extent.width().div_ceil(TILE_EDGE));
        let region = self.extent.tile_region((index % columns) as u32, (index / columns) as u32)?;
        let outside = (0..TILE_EDGE).any(|row| {
            let start = tile_index(0, row);
            let line = &values[start..start + TILE_EDGE as usize];
            if row >= region.height() {
                line.iter().any(|value| *value != 0)
            } else {
                line[region.width() as usize..].iter().any(|value| *value != 0)
            }
        });
        if outside {
            return Err(ImageError::Invalid(
                "Stored coverage tile covers pixels outside its extent",
            ));
        }
        if uniform_value(&values, region).is_some() {
            return Err(ImageError::Invalid("Stored coverage tile holds a single value"));
        }
        self.slots.insert(index, Slot::Stored(Arc::new(CoverageTile::new(values, memory)?)));
        Ok(())
    }

    /// Complete the coverage.
    /// # Errors
    /// Preserves memory pressure.
    pub fn finish(self, memory: &MemoryLease) -> Result<Coverage, ImageError> {
        Ok(Coverage {
            extent: self.extent,
            columns: self.extent.width().div_ceil(TILE_EDGE),
            tiles: Arc::new(Tiles::new(self.extent, self.default, self.slots, memory)?),
        })
    }

    fn admit(&mut self, index: u64) -> Result<(), ImageError> {
        if index < self.next || index >= self.tiles {
            return Err(ImageError::Invalid("Coverage tile index is out of order or off the grid"));
        }
        self.next = index + 1;
        Ok(())
    }
}

fn tile_count(extent: Extent) -> u64 {
    u64::from(extent.width().div_ceil(TILE_EDGE)) * u64::from(extent.height().div_ceil(TILE_EDGE))
}
