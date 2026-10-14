// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one reusable slot occupied through every native registration alias.
use super::{BackendCustodyCleanup, BackendCustodyMemory, BackendCustodySlotMemory};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{AdmissionError, Moment};

/// Closed one-turn custody; uncertain output owners retain this identical turn.
/// Final control deallocation precedes releasing the original slot and metadata.
pub struct BackendCustodySlotTurn {
    owner: Option<Arc<Turn>>,
    memory: BackendCustodySlotMemory,
}
struct Turn {
    cleanup: Mutex<Option<Moment>>,
    memory: BackendCustodySlotMemory,
}
/// Construct only after original slot issuance and successful exclusive occupation.
pub(super) fn new(memory: BackendCustodySlotMemory) -> BackendCustodySlotTurn {
    BackendCustodySlotTurn {
        owner: Some(Arc::new(Turn { cleanup: Mutex::new(None), memory: memory.clone() })),
        memory,
    }
}
impl BackendCustodySlotTurn {
    pub(super) fn index(&self) -> Option<usize> {
        self.memory.index()
    }
    /// Return the original retained bytes; this fact grants no new claim.
    pub const fn bytes(&self) -> u64 {
        self.memory.bytes()
    }
    /// Include this actual control layout in the fixed slot's native preflight.
    pub const fn control_storage_bytes() -> usize {
        size_of::<Turn>() + 2 * size_of::<usize>()
    }
    pub(crate) fn metadata(&self) -> &BackendCustodyMemory {
        self.memory.metadata()
    }
    /// Compare only the actual original turn allocation without invoking providers.
    pub fn same_allocation(&self, other: &Self) -> bool {
        match (&self.owner, &other.owner) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    /// Latch this turn's first close-only cutoff and arm its existing timer slot.
    /// # Errors
    /// Preserves actual provider/arithmetic failures and never renews a cutoff.
    pub fn begin_cleanup(
        &self,
        original_until: Moment,
    ) -> Result<BackendCustodyCleanup, AdmissionError> {
        let now = self.metadata().observe_retirement()?;
        let owner = self.owner.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let until = super::memory::latch(&owner.cleanup, now, original_until)?;
        self.memory.arm_turn(until)?;
        Ok(BackendCustodyCleanup {
            metadata: self.metadata().clone(),
            slot: Some(self.clone()),
            until,
        })
    }
}
impl Clone for BackendCustodySlotTurn {
    fn clone(&self) -> Self {
        Self { owner: self.owner.clone(), memory: self.memory.clone() }
    }
}
impl Drop for BackendCustodySlotTurn {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            drop(Arc::into_inner(owner));
        }
    }
}
impl Drop for Turn {
    fn drop(&mut self) {
        self.memory.finish_turn();
    }
}
