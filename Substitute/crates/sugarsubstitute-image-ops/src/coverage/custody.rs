// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move coverage storage into the lease of the owner that keeps it.
//!
//! Work computes coverage under a job's own reservation. A document that keeps the result in
//! its content and history must charge it to the document's lease instead, or every retained
//! result would keep its job's whole working reservation admitted after the job ended.
use super::{Coverage, CoverageTile, Slot, TILE_VALUES, Tiles, beyond::Beyond};
use crate::ImageError;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::MemoryLease;

impl Coverage {
    /// The same coverage with all of its storage charged to `memory`.
    ///
    /// Storage already charged there is shared unchanged. Storage this value alone holds keeps
    /// its allocation and only moves its charge; storage shared with other owners is copied
    /// under a new charge, so those owners keep theirs. Values and fingerprints are unchanged.
    /// # Errors
    /// Rejects insufficient memory in `memory`; on failure nothing is charged to it.
    pub fn adopt(self, memory: &MemoryLease) -> Result<Self, ImageError> {
        let tiles = if self.tiles.charged_to(memory) {
            self.tiles
        } else {
            Arc::new(
                Arc::try_unwrap(self.tiles).map_or_else(Held::Shared, Held::Owned).adopt(memory)?,
            )
        };
        let beyond = match self.beyond {
            Some(beyond) if !beyond.frame.charged_to(memory) => {
                let Beyond { origin, frame } = unwrap_or_copy_beyond(beyond);
                Some(Arc::new(Beyond { origin, frame: frame.adopt(memory)? }))
            }
            kept => kept,
        };
        Ok(Self { tiles, beyond, ..self })
    }

    /// Whether every allocation of this coverage is charged to `memory`.
    pub fn charged_to(&self, memory: &MemoryLease) -> bool {
        self.tiles.charged_to(memory)
            && self.beyond.as_ref().is_none_or(|beyond| beyond.frame.charged_to(memory))
    }
}

/// Tile storage this value holds alone, or a share of storage other owners still hold.
enum Held {
    Owned(Tiles),
    Shared(Arc<Tiles>),
}

impl Held {
    /// Charge the tile map and every stored tile to `memory`.
    fn adopt(self, memory: &MemoryLease) -> Result<Tiles, ImageError> {
        let (default, slots, fingerprint, charge) = match self {
            Self::Owned(tiles) if tiles._memory.charged_to(memory) => {
                (tiles.default, tiles.slots, tiles.fingerprint, Some(tiles._memory))
            }
            Self::Owned(tiles) => (tiles.default, tiles.slots, tiles.fingerprint, None),
            Self::Shared(tiles) => (tiles.default, tiles.slots.clone(), tiles.fingerprint, None),
        };
        let charge = match charge {
            Some(charge) => charge,
            None => memory.claim(Tiles::charge_bytes(slots.len()))?,
        };
        let mut adopted = std::collections::BTreeMap::new();
        for (key, slot) in slots {
            let slot = match slot {
                Slot::Stored(tile) if !tile._memory.charged_to(memory) => {
                    Slot::Stored(Arc::new(adopt_tile(tile, memory)?))
                }
                kept => kept,
            };
            adopted.insert(key, slot);
        }
        Ok(Tiles { default, slots: adopted, fingerprint, _memory: charge })
    }
}

/// One tile charged to `memory`: its own allocation when nothing else holds it, else a copy.
fn adopt_tile(tile: Arc<CoverageTile>, memory: &MemoryLease) -> Result<CoverageTile, ImageError> {
    let charge = memory.claim(CoverageTile::CHARGE_BYTES)?;
    match Arc::try_unwrap(tile) {
        Ok(CoverageTile { values, fingerprint, .. }) => {
            Ok(CoverageTile { values, fingerprint, _memory: charge })
        }
        Err(shared) => {
            let mut values = Vec::new();
            values.try_reserve_exact(TILE_VALUES)?;
            values.extend_from_slice(&shared.values);
            Ok(CoverageTile { values, fingerprint: shared.fingerprint, _memory: charge })
        }
    }
}

/// Content kept beyond the extent, taken from this value or copied from a shared holder.
fn unwrap_or_copy_beyond(beyond: Arc<Beyond>) -> Beyond {
    Arc::try_unwrap(beyond)
        .unwrap_or_else(|shared| Beyond { origin: shared.origin, frame: shared.frame.clone() })
}

impl Tiles {
    /// Whether the map and every stored tile are charged to `memory`.
    fn charged_to(&self, memory: &MemoryLease) -> bool {
        self._memory.charged_to(memory)
            && self.slots.values().all(|slot| match slot {
                Slot::Uniform(_) => true,
                Slot::Stored(tile) => tile._memory.charged_to(memory),
            })
    }
}
