// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fund witness storage from the actual fixed worker pool before constructing native owners.
use crate::ClaimError;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_execution::backend_custody::{BackendCustodyMemory, BackendCustodySlotMemory};

/// Closed metadata alias; no raw Arc/Weak escapes the witness owner.
pub(crate) struct WitnessPool(Option<Arc<Pool>>);
struct Pool {
    slots: [Slot; 128],
    metadata: BackendCustodyMemory,
}
struct Slot {
    used: AtomicBool,
    memory: BackendCustodySlotMemory,
}
impl WitnessPool {
    /// Issue the existing128 slot credits from the genuine same-worker producer.
    pub(crate) fn new(metadata: &BackendCustodyMemory) -> Result<Self, ClaimError> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(128)
            .map_err(|cause| ClaimError::new("custody_pool_capacity", cause))?;
        for _ in 0..128 {
            slots.push(Slot {
                used: AtomicBool::new(false),
                memory: metadata
                    .claim_slot()
                    .map_err(|cause| ClaimError::new("custody_pool_slot", cause))?,
            });
        }
        let slots = slots.try_into().map_err(|_| ClaimError::invalid("custody_pool_capacity"))?;
        Ok(Self(Some(Arc::new(Pool { slots, metadata: metadata.clone() }))))
    }
    /// Occupy one of the bounded64 backend registrations; other slots stay unissued.
    pub(crate) fn acquire(&self) -> Result<Option<WitnessSlot>, ClaimError> {
        let pool = self.0.as_ref().ok_or_else(|| ClaimError::invalid("custody_pool_closed"))?;
        for (index, slot) in pool.slots.iter().enumerate().take(64) {
            if slot.used.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err()
            {
                continue;
            }
            let turn = match slot.memory.begin_turn() {
                Ok(turn) => turn,
                Err(cause) => {
                    slot.used.store(false, Ordering::Release);
                    return Err(ClaimError::new("custody_slot_turn", cause));
                }
            };
            return Ok(Some(WitnessSlot(Some(Arc::new(SlotUse {
                pool: self.clone(),
                index,
                phase: std::sync::Mutex::new(Phase::Uninitialized),
                cleanup: std::sync::Mutex::new(None),
                turn,
            })))));
        }
        Ok(None)
    }
}
impl Clone for WitnessPool {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for WitnessPool {
    fn drop(&mut self) {
        if let Some(pool) = self.0.take() {
            drop(Arc::into_inner(pool));
        }
    }
}
/// Keep the complete slot/control allocation charged through every native alias.
pub(crate) struct WitnessSlot(Option<Arc<SlotUse>>);
struct SlotUse {
    pool: WitnessPool,
    index: usize,
    phase: std::sync::Mutex<Phase>,
    cleanup:
        std::sync::Mutex<Option<sugarsubstitute_execution::backend_custody::BackendCustodyCleanup>>,
    turn: sugarsubstitute_execution::backend_custody::BackendCustodySlotTurn,
}
#[derive(Clone, Copy)]
enum Phase {
    Uninitialized,
    Preparing(sugarsubstitute_contracts::execution::Moment),
    Active,
    Cleaning(sugarsubstitute_contracts::execution::Moment),
    Retired,
}
impl WitnessSlot {
    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub(crate) const fn bytes(&self) -> u64 {
        512 * 1024
    }
    /// Install only the first authenticated original preparation cutoff.
    pub(crate) fn prepare(
        &self,
        until: sugarsubstitute_contracts::execution::Moment,
    ) -> Result<(), ClaimError> {
        let slot = self.0.as_ref().ok_or_else(|| ClaimError::invalid("custody_slot_closed"))?;
        let mut phase = super::super::lock(&slot.phase);
        if !matches!(*phase, Phase::Uninitialized) {
            return Err(ClaimError::invalid("custody_slot_phase"));
        }
        *phase = Phase::Preparing(until);
        Ok(())
    }
    /// Commit only after the same actual suspended root was adopted and acknowledged.
    pub(crate) fn root_adopted(&self) -> Result<(), ClaimError> {
        self.check()?;
        let slot = self.0.as_ref().ok_or_else(|| ClaimError::invalid("custody_slot_closed"))?;
        let mut phase = super::super::lock(&slot.phase);
        if !matches!(*phase, Phase::Preparing(_)) {
            return Err(ClaimError::invalid("custody_slot_phase"));
        }
        *phase = Phase::Active;
        Ok(())
    }
    /// Latch the original turn's one cleanup proof before any native close.
    pub(crate) fn cleanup(
        &self,
        original_until: sugarsubstitute_contracts::execution::Moment,
    ) -> Result<(), ClaimError> {
        let slot = self.0.as_ref().ok_or_else(|| ClaimError::invalid("custody_slot_closed"))?;
        let proof = slot
            .turn
            .begin_cleanup(original_until)
            .map_err(|cause| ClaimError::new("custody_cleanup_control", cause))?;
        let until = proof.until();
        {
            let mut held = super::super::lock(&slot.cleanup);
            if held.is_none() {
                *held = Some(proof);
            }
        }
        let mut phase = super::super::lock(&slot.phase);
        if !matches!(*phase, Phase::Cleaning(_) | Phase::Retired) {
            *phase = Phase::Cleaning(until);
        }
        Ok(())
    }
    /// Borrow the original occupied turn for its native process observations.
    pub(crate) fn turn(
        &self,
    ) -> Result<&sugarsubstitute_execution::backend_custody::BackendCustodySlotTurn, ClaimError>
    {
        self.0
            .as_ref()
            .map(|slot| &slot.turn)
            .ok_or_else(|| ClaimError::invalid("custody_slot_closed"))
    }
    /// Clone only the first original close proof; no cutoff/claim is recreated.
    pub(crate) fn cleanup_proof(
        &self,
    ) -> Result<sugarsubstitute_execution::backend_custody::BackendCustodyCleanup, ClaimError> {
        let slot = self.0.as_ref().ok_or_else(|| ClaimError::invalid("custody_slot_closed"))?;
        super::super::lock(&slot.cleanup)
            .clone()
            .ok_or_else(|| ClaimError::invalid("custody_cleanup_missing"))
    }
    /// Gate every fresh kernel/file effect using the genuine original control.
    /// Native return adoption happens before the next check; no provider runs under locks.
    pub(crate) fn check(&self) -> Result<(), ClaimError> {
        let slot = self.0.as_ref().ok_or_else(|| ClaimError::invalid("custody_slot_closed"))?;
        let phase = *super::super::lock(&slot.phase);
        let pool =
            slot.pool.0.as_ref().ok_or_else(|| ClaimError::invalid("custody_pool_closed"))?;
        if matches!(phase, Phase::Cleaning(_)) {
            return self
                .cleanup_proof()?
                .check()
                .map_err(|cause| ClaimError::new("custody_original_control", cause));
        }
        let now = pool
            .metadata
            .check_registration()
            .map_err(|cause| ClaimError::new("custody_original_control", cause))?;
        match phase {
            Phase::Preparing(until) | Phase::Cleaning(until) if now < until => Ok(()),
            Phase::Active => Ok(()),
            _ => Err(ClaimError::invalid("custody_original_cutoff")),
        }
    }
    /// Mark only after actual native/file outputs settled; this grants no new effects.
    pub(crate) fn retired(&self) {
        if let Some(slot) = &self.0 {
            *super::super::lock(&slot.phase) = Phase::Retired;
        }
    }
}
impl Clone for WitnessSlot {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for WitnessSlot {
    fn drop(&mut self) {
        if let Some(slot) = self.0.take() {
            drop(Arc::into_inner(slot));
        }
    }
}
impl Drop for SlotUse {
    fn drop(&mut self) {
        if let Some(pool) = &self.pool.0 {
            pool.slots[self.index].used.store(false, Ordering::Release);
        }
    }
}
