// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share bounded semantic tile retention between CPU and GPU evaluators.
use crate::{ImageError, Region};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

struct Entry<T> {
    key: [u8; 32],
    region: Region,
    used: u64,
    retention: u8,
    tile: Arc<T>,
}
/// Process-local references to immutable backend tiles, keyed by graph content and exact region.
/// This policy obtains its storage from execution; it creates no admission or persistent cache.
/// Outside references retain their allocations after eviction. Backends own native retirement.
pub struct TileRetention<T> {
    entries: Vec<Entry<T>>,
    maximum: usize,
    tick: u64,
    preferred: u8,
    _memory: MemoryClaim,
}
impl<T> TileRetention<T> {
    /// Reserve bounded metadata using the evaluator's original execution lease.
    /// # Errors
    /// Rejects zero/excessive capacity, memory pressure and allocation failure.
    pub fn new(maximum: usize, memory: &MemoryLease) -> Result<Self, ImageError> {
        if maximum == 0 || maximum > 4096 {
            return Err(ImageError::Invalid("Invalid tile retention limit"));
        }
        let claim = memory.claim((maximum * size_of::<Entry<T>>()) as u64)?;
        let mut entries = Vec::new();
        entries.try_reserve_exact(maximum)?;
        Ok(Self { entries, maximum, tick: 0, preferred: 0, _memory: claim })
    }
    /// Set the outer request's canonical pyramid level before recursive evaluation.
    pub fn prefer_level(&mut self, level: u8) {
        self.preferred = level;
    }
    /// Number of references retained by the evaluator, excluding outside owners.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether the evaluator retains no tiles.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Release all evaluator references; outside owners remain charged.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    /// Reuse exactly matching content/region and update its recency and pyramid provenance.
    pub fn find(&mut self, key: [u8; 32], region: Region, level: Option<u8>) -> Option<Arc<T>> {
        self.tick = self.tick.saturating_add(1);
        let entry =
            self.entries.iter_mut().find(|entry| entry.key == key && entry.region == region)?;
        entry.used = self.tick;
        entry.retention = entry.retention.max(level.map_or(0, |level| level.saturating_add(1)));
        Some(entry.tile.clone())
    }
    /// Retain a completed immutable tile, evicting according to the same finite pyramid quotas.
    pub fn insert(&mut self, key: [u8; 32], region: Region, level: Option<u8>, tile: Arc<T>) {
        if self.entries.len() == self.maximum
            && let Some(index) = self.victim(|_| true)
        {
            self.entries.swap_remove(index);
        }
        self.entries.push(Entry {
            key,
            region,
            used: self.tick,
            retention: level.map_or(0, |level| level.saturating_add(1)),
            tile,
        });
    }
    /// Evict one cache-only reference under allocation pressure, including aliases of one tile.
    /// Returns false when every candidate has outside or backend allocation references.
    /// The backend reports additional native allocation references beyond this tile's Arc.
    /// Submitted work and outside results must retain their allocation guards until completion.
    pub fn evict_unpinned(&mut self, backend_unpinned: impl Fn(&T) -> bool) -> bool {
        let Some(index) = self.victim(|tile| backend_unpinned(tile) && self.solely_cached(tile))
        else {
            return false;
        };
        self.entries.swap_remove(index);
        true
    }
    /// Alias entries share an allocation; their own references do not constitute an outside pin.
    fn solely_cached(&self, tile: &Arc<T>) -> bool {
        let references = Arc::strong_count(tile);
        references == 1
            || (references <= self.entries.len()
                && self
                    .entries
                    .iter()
                    .filter(|entry| Arc::ptr_eq(&entry.tile, tile))
                    .take(references)
                    .count()
                    == references)
    }
    fn victim(&self, eligible: impl Fn(&Arc<T>) -> bool) -> Option<usize> {
        let mut counts = [0_usize; 256];
        for entry in &self.entries {
            counts[usize::from(entry.retention)] += 1;
        }
        let preferred = usize::from(self.preferred.saturating_add(1));
        let capacity = self.entries.len();
        let target = if preferred == 1 { capacity } else { (capacity / 2).max(1) };
        let finer = if preferred == 1 {
            0
        } else {
            (capacity - counts[preferred].min(target)) / (preferred - 1)
        };
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| eligible(&entry.tile))
            .min_by_key(|(_, entry)| {
                let level = usize::from(entry.retention);
                let quota = if level == preferred { target } else { finer };
                let priority = if level == 0 {
                    0
                } else if level > preferred {
                    1
                } else if counts[level] > quota {
                    2
                } else if level == preferred {
                    4
                } else {
                    3
                };
                (priority, entry.used)
            })
            .map(|(index, _)| index)
    }
}
