// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one admitted charge separately from terminal-delivery credits.
use crate::{CLAIM_POOL_BYTES, ClaimError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
pub(crate) const SLOT_BYTES: u64 = 512 * 1024;
pub(crate) struct Pool {
    slots: [Slot; 128],
    _core: MemoryClaim,
}
struct Slot {
    used: AtomicBool,
    _claim: MemoryClaim,
}
impl Pool {
    pub(crate) fn new(lease: &MemoryLease) -> Result<Arc<Self>, ClaimError> {
        if lease.bytes() != CLAIM_POOL_BYTES {
            return Err(ClaimError::invalid("pool_reservation"));
        }
        let core = lease.claim(2 * 1024 * 1024).map_err(|e| ClaimError::new("pool_core", e))?;
        let mut slots = Vec::new();
        slots.try_reserve_exact(128).map_err(|e| ClaimError::new("pool_slots", e))?;
        for _ in 0..128 {
            slots.push(Slot {
                used: AtomicBool::new(false),
                _claim: lease.claim(SLOT_BYTES).map_err(|e| ClaimError::new("pool_slot", e))?,
            });
        }
        let slots = slots.try_into().map_err(|_| ClaimError::invalid("pool_slot_count"))?;
        Ok(Arc::new(Self { slots, _core: core }))
    }
    pub(crate) fn acquire(self: &Arc<Self>) -> Option<Arc<SlotUse>> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.used
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .ok()
                .map(|_| Arc::new(SlotUse { pool: self.clone(), index }))
        })
    }
}
pub(crate) struct SlotUse {
    pool: Arc<Pool>,
    index: usize,
}
impl Drop for SlotUse {
    fn drop(&mut self) {
        self.pool.slots[self.index].used.store(false, Ordering::Release);
    }
}
