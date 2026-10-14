// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Latch original close-only cutoffs and schedule genuine lock-contention observations.
use super::*;
/// Close-only original control for one actual helper or fixed native slot.
/// It carries no raw handle/ledger factory; each native owner checks its exact
/// metadata/slot allocation before invoking original controlled retirement.
#[derive(Clone)]
pub struct BackendCustodyCleanup {
    pub(crate) metadata: BackendCustodyMemory,
    pub(crate) slot: Option<crate::backend_custody::BackendCustodySlotTurn>,
    pub(crate) until: Moment,
}
impl BackendCustodyCleanup {
    /// Schedule a bounded observation only after genuine writer-lock contention.
    /// It uses this same existing timer slot and never extends the first cutoff.
    /// # Errors
    /// Preserves original control/arithmetic failure or expired actual cleanup.
    pub fn observe_contention(&self) -> Result<(), AdmissionError> {
        self.check()?;
        let now = self.metadata.observe_retirement()?;
        let at = now
            .checked_add(std::time::Duration::from_millis(250))
            .map_err(|cause| {
                AdmissionError::Unexpected(
                    sugarsubstitute_contracts::execution::ExecutionError::new(
                        sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
                        Box::new(cause),
                    ),
                )
            })?
            .min(self.until);
        let owner = self.metadata.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let index = self.slot.as_ref().and_then(|slot| slot.index()).unwrap_or(64);
        {
            let mut state = crate::scope::lock(&core.state);
            let record =
                state.jobs.get_mut(&owner.context.job_id()).ok_or(AdmissionError::ShuttingDown)?;
            if record.phase != JobPhase::Running
                || record
                    .memory
                    .as_ref()
                    .is_none_or(|memory| !memory.matches_lease(owner.context.memory()))
            {
                return Err(AdmissionError::ShuttingDown);
            }
            if !record.custody_deadlines.as_mut().is_some_and(|slots| slots.progress(index, at)) {
                return Err(AdmissionError::DeadlineExpired { deadline: self.until, now });
            }
        }
        core.changed();
        Ok(())
    }
    /// Observe only the original timer's contention fact; this grants no native proof.
    pub fn contention_observed(&self) -> bool {
        let Some(owner) = &self.metadata.0 else {
            return false;
        };
        let Some(core) = owner.core.upgrade() else {
            return false;
        };
        let index = self.slot.as_ref().and_then(|slot| slot.index()).unwrap_or(64);
        let state = crate::scope::lock(&core.state);
        state
            .jobs
            .get(&owner.context.job_id())
            .and_then(|record| record.custody_deadlines.as_ref())
            .is_some_and(|slots| slots.progress_due(index))
    }
    /// Return the first immutable local cutoff for observation and timer ownership.
    pub const fn until(&self) -> Moment {
        self.until
    }
    /// Check the actual original worker/clock before a fresh close-only effect.
    /// # Errors
    /// Preserves actual provider failure and original expiration without renewal.
    pub fn check(&self) -> Result<(), AdmissionError> {
        let now = self.metadata.observe_retirement()?;
        if now >= self.until {
            Err(AdmissionError::DeadlineExpired { deadline: self.until, now })
        } else {
            Ok(())
        }
    }
    pub(crate) fn matches_metadata(&self, original: &BackendCustodyMemory) -> bool {
        match (&self.metadata.0, &original.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub(crate) fn matches_slot(
        &self,
        original: &crate::backend_custody::BackendCustodySlotTurn,
    ) -> bool {
        self.slot.as_ref().is_some_and(|slot| slot.same_allocation(original))
    }
}
/// Freeze once using an actual sampled source, keeping arithmetic/provider calls outside locks.
pub(crate) fn latch(
    slot: &std::sync::Mutex<Option<Moment>>,
    now: Moment,
    original_until: Moment,
) -> Result<Moment, AdmissionError> {
    if let Some(until) = *crate::scope::lock(slot) {
        return Ok(until);
    }
    let proposed = now
        .checked_add(std::time::Duration::from_secs(15))
        .map_err(|cause| {
            AdmissionError::Unexpected(sugarsubstitute_contracts::execution::ExecutionError::new(
                sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
                Box::new(cause),
            ))
        })?
        .min(original_until);
    let mut slot = crate::scope::lock(slot);
    let until = *slot.get_or_insert(proposed);
    Ok(until)
}
