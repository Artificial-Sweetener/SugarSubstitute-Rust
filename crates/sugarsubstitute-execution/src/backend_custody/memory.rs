// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the original fixed custody metadata charge through final control deallocation.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, JobPhase, MemoryClaim, MemoryClaimError, Moment,
};

/// Fixed metadata credit minted only by the actual admitted custody worker.
/// Clones share one original two-MiB claim. This owner exposes no raw claim,
/// Arc, Weak or numerical allocation constructor.
pub struct BackendCustodyMemory(Option<Arc<Metadata>>);
struct Metadata {
    issued: AtomicUsize,
    source_issued: std::sync::atomic::AtomicBool,
    cleanup: std::sync::Mutex<Option<Moment>>,
    key: sugarsubstitute_contracts::backend_custody::BackendCustodyKey,
    parent: (u32, u64),
    source: crate::lifecycle_pipe::SharedRef,
    source_owner: crate::lifecycle_pipe::Owner,
    context: JobContext,
    core: std::sync::Weak<crate::admission::Core>,
    _claim: MemoryClaim,
}
impl BackendCustodyMemory {
    /// Match only the actual authenticated source session allocation.
    pub fn matches_session(&self, control: &super::BackendCustodyPipeControl) -> bool {
        self.0.as_ref().is_some_and(|owner| owner.source.same_allocation(&control.inner.shared))
    }
    /// Begin the one funded durable-source construction before driver binding.
    /// Queued original work permits inert allocation only; native effects still
    /// require the actual running record. Every subsequent attempt rejects inline.
    /// # Errors
    /// Preserves the first real provider/admission failure without renewing issuance.
    pub fn begin_source(&self) -> Result<(), AdmissionError> {
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        if owner
            .source_issued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(AdmissionError::NativeResources(
                sugarsubstitute_contracts::execution::NativeResourceError::OriginalAdmissionRetired,
            ));
        }
        self.observe_at(false, true).map(|_| ())
    }
    /// Conservatively translate authenticated QPC facts into this original worker epoch.
    /// The native counter and original Core clock run outside owned locks. Facts
    /// can shorten a preparation/cleanup cutoff and confer no native ownership.
    /// # Errors
    /// Preserves original counter, provider or current admission rejection.
    #[cfg(windows)]
    pub fn decode_counter(
        &self,
        source: sugarsubstitute_contracts::execution::CounterDeadline,
        retiring: bool,
    ) -> Result<Moment, crate::native::NativeObservationError> {
        let before = self.observe(retiring).map_err(|cause| {
            crate::native::NativeObservationError::new("custody_counter_original", cause)
        })?;
        let sample = crate::windows_counter::sample()?;
        let until = crate::counter_deadline::decode(before, sample, source)?;
        self.observe(retiring).map_err(|cause| {
            crate::native::NativeObservationError::new("custody_counter_post", cause)
        })?;
        Ok(until)
    }
    /// Compare only the sealed original allocation without sampling providers.
    pub fn same_allocation(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    /// Return compiled producer/control bytes for original metadata preflight.
    pub const fn control_storage_bytes() -> usize {
        size_of::<Metadata>()
            + 2 * size_of::<usize>()
            + size_of::<super::deadlines::Deadlines>()
            + size_of::<crate::admission::Record>()
    }
    /// Allocate one distinct process-signal cell on this exact original native owner.
    pub(crate) fn watch_signal(&self) -> Option<Arc<crate::native::Signal>> {
        self.0
            .as_ref()
            .and_then(|owner| owner.core.upgrade())
            .map(|core| crate::native::Signal::new(&core))
    }
    pub(crate) fn matches_wake(&self, wake: &super::BackendCustodyWake) -> bool {
        self.0.as_ref().is_some_and(|owner| {
            owner.context.job_id() == wake.job && wake.signal.matches_core(&owner.core)
        })
    }
    /// Notify only the original admitted worker after its private binding changed.
    pub(crate) fn changed(&self) {
        if let Some(core) = self.0.as_ref().and_then(|owner| owner.core.upgrade()) {
            core.changed();
        }
    }
    pub(crate) fn new(
        claim: MemoryClaim,
        context: JobContext,
        key: sugarsubstitute_contracts::backend_custody::BackendCustodyKey,
        parent: (u32, u64),
        core: std::sync::Weak<crate::admission::Core>,
        source: crate::lifecycle_pipe::SharedRef,
        source_owner: crate::lifecycle_pipe::Owner,
    ) -> Self {
        Self(Some(Arc::new(Metadata {
            issued: AtomicUsize::new(0),
            source_issued: std::sync::atomic::AtomicBool::new(false),
            cleanup: std::sync::Mutex::new(None),
            key,
            parent,
            source,
            source_owner,
            context,
            core,
            _claim: claim,
        })))
    }
    /// Return the fixed retained metadata bytes as accounting facts only.
    pub const fn bytes(&self) -> u64 {
        2 * 1024 * 1024
    }
    /// Borrow the original real authenticated child-session association.
    pub fn key(&self) -> Option<sugarsubstitute_contracts::backend_custody::BackendCustodyKey> {
        self.0.as_ref().map(|owner| owner.key)
    }
    /// Return actual original kernel-authenticated parent PID and creation time facts.
    /// Native adoption must still validate the identical transferred process handle.
    pub fn parent_identity(&self) -> Option<(u32, u64)> {
        self.0.as_ref().map(|owner| owner.parent)
    }
    /// Borrow the genuine original worker context; caller contexts never replace it.
    pub fn original_context(&self) -> Option<&JobContext> {
        self.0.as_ref().map(|owner| &owner.context)
    }
    /// Recheck actual running worker/ledger and current complete scope ancestry.
    /// The actual Core clock runs before all owned locks. This grants no handles.
    /// # Errors
    /// Preserves the original provider cause or actual closed/expired admission.
    pub fn check_registration(&self) -> Result<Moment, AdmissionError> {
        self.observe(false)
    }
    /// Observe original actual worker identity/clock for releasing already-owned custody.
    /// Cancellation closes registration but does not manufacture native settlement.
    /// # Errors
    /// Rejects a missing/terminal/foreign worker or actual fatal clock/provider failure.
    pub fn observe_retirement(&self) -> Result<Moment, AdmissionError> {
        self.observe(true)
    }
    /// Latch one close-only cutoff for this actual helper observation owner.
    /// The supplied original source cutoff can only shorten the fixed15 seconds.
    /// # Errors
    /// Preserves actual clock/arithmetic failure and never renews a latched cutoff.
    pub fn begin_cleanup(
        &self,
        original_until: Moment,
    ) -> Result<BackendCustodyCleanup, AdmissionError> {
        let now = self.observe_retirement()?;
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let until = latch(&owner.cleanup, now, original_until)?;
        self.arm_cleanup(64, until)?;
        Ok(BackendCustodyCleanup { metadata: self.clone(), slot: None, until })
    }
    /// Observe only the actual queued cutoff fact for this original helper.
    pub fn cleanup_due(&self) -> bool {
        let Some(owner) = &self.0 else {
            return false;
        };
        let Some(core) = owner.core.upgrade() else {
            return false;
        };
        let state = crate::scope::lock(&core.state);
        state
            .jobs
            .get(&owner.context.job_id())
            .and_then(|record| record.custody_deadlines.as_ref())
            .is_some_and(|slots| slots.due(64))
    }
    /// Arm one fixed original cutoff with no new timer/job or provider callback.
    fn arm_cleanup(&self, index: usize, until: Moment) -> Result<(), AdmissionError> {
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
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
            record
                .custody_deadlines
                .as_mut()
                .ok_or(AdmissionError::ShuttingDown)?
                .arm(index, until);
        }
        core.changed();
        Ok(())
    }
    /// Change only an occupied recovery turn's original timer stage.
    fn recovery_timer(
        &self,
        index: usize,
        now: Moment,
        until: Moment,
        begin: bool,
    ) -> Result<(), AdmissionError> {
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
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
            let slots = record.custody_deadlines.as_mut().ok_or(AdmissionError::ShuttingDown)?;
            let accepted = if begin {
                slots.capture(index, until)
            } else {
                slots.after_capture(index, now, until)
            };
            if !accepted {
                return Err(AdmissionError::NativeResources(sugarsubstitute_contracts::execution::NativeResourceError::OriginalAdmissionRetired));
            }
        }
        core.changed();
        Ok(())
    }
    fn observe(&self, retiring: bool) -> Result<Moment, AdmissionError> {
        self.observe_at(retiring, false)
    }
    /// Observe the original queued/running source before inert construction only.
    fn observe_at(&self, retiring: bool, constructing: bool) -> Result<Moment, AdmissionError> {
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = crate::scope::lock(&core.state);
        let record = state.jobs.get(&owner.context.job_id()).ok_or(AdmissionError::ShuttingDown)?;
        if (record.phase != JobPhase::Running
            && !(constructing && record.phase == JobPhase::Queued))
            || (!retiring && record.retiring)
            || record.scope_id != owner.context.scope_id()
            || record
                .memory
                .as_ref()
                .is_none_or(|memory| !memory.matches_lease(owner.context.memory()))
        {
            return Err(AdmissionError::ShuttingDown);
        }
        if retiring {
            return Ok(now);
        }
        crate::lifecycle_pipe::current_admission::source_locked(
            &core,
            &state,
            &owner.source,
            &owner.source_owner,
            now,
        )?;
        let deadline = crate::admission_liveness::scope_deadline(&core, &state, record)?;
        if let Some(deadline) = deadline.filter(|deadline| *deadline <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        Ok(now)
    }
    /// Issue one of the original128 fixed512-KiB slots from the actual same ledger.
    /// This accepts no caller context/bytes and exposes neither lease nor claim.
    /// # Errors
    /// Rejects exhausted slot issuance or actual original ledger capacity.
    pub fn claim_slot(&self) -> Result<BackendCustodySlotMemory, MemoryClaimError> {
        let Some(owner) = &self.0 else {
            return Err(MemoryClaimError { requested_bytes: 512 * 1024, available_bytes: 0 });
        };
        let index = owner
            .issued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |issued| {
                (issued < 128).then_some(issued + 1)
            })
            .map_err(|_| MemoryClaimError { requested_bytes: 512 * 1024, available_bytes: 0 })?;
        let claim = owner.context.memory().claim(512 * 1024)?;
        Ok(BackendCustodySlotMemory {
            claim: Some(Arc::new(SlotCredit {
                index,
                active: std::sync::atomic::AtomicBool::new(false),
                _claim: claim,
            })),
            metadata: self.clone(),
        })
    }
}
impl Clone for BackendCustodyMemory {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Retain one fixed original512-KiB slot and its actual metadata allocation.
/// Every clone carries the same claim; no numerical/public factory exists.
pub struct BackendCustodySlotMemory {
    claim: Option<Arc<SlotCredit>>,
    metadata: BackendCustodyMemory,
}
struct SlotCredit {
    index: usize,
    active: std::sync::atomic::AtomicBool,
    _claim: MemoryClaim,
}
impl BackendCustodySlotMemory {
    pub(super) fn index(&self) -> Option<usize> {
        self.claim.as_ref().map(|owner| owner.index)
    }
    pub(crate) fn metadata(&self) -> &BackendCustodyMemory {
        &self.metadata
    }
    /// Return the original fixed slot bytes as inert accounting facts.
    pub const fn bytes(&self) -> u64 {
        512 * 1024
    }
    /// Begin one bounded native registration on this original reusable slot.
    /// All aliases of the prior turn must be destroyed before another can begin.
    /// Uncertain native custody retains its turn and keeps this slot occupied.
    /// # Errors
    /// Rejects concurrent reuse, non-backend slots or closed original admission.
    pub fn begin_turn(&self) -> Result<super::BackendCustodySlotTurn, AdmissionError> {
        self.metadata.check_registration()?;
        let owner = self.claim.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        if owner.index >= 64
            || owner
                .active
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err(AdmissionError::NativeResources(
                sugarsubstitute_contracts::execution::NativeResourceError::OriginalAdmissionRetired,
            ));
        }
        Ok(super::turn::new(self.clone()))
    }
    pub(super) fn arm_turn(&self, until: Moment) -> Result<(), AdmissionError> {
        let owner = self.claim.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        self.metadata.arm_cleanup(owner.index, until)
    }
    pub(super) fn recovery_timer(
        &self,
        now: Moment,
        until: Moment,
        begin: bool,
    ) -> Result<(), AdmissionError> {
        let owner = self.claim.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        self.metadata.recovery_timer(owner.index, now, until, begin)
    }
    pub(super) fn finish_turn(&self) {
        let Some(owner) = &self.claim else {
            return;
        };
        if let Some(metadata) = &self.metadata.0
            && let Some(core) = metadata.core.upgrade()
        {
            let mut state = crate::scope::lock(&core.state);
            if let Some(slots) = state
                .jobs
                .get_mut(&metadata.context.job_id())
                .and_then(|record| record.custody_deadlines.as_mut())
            {
                slots.disarm(owner.index);
            }
        }
        owner.active.store(false, Ordering::Release);
    }
}
impl Clone for BackendCustodySlotMemory {
    fn clone(&self) -> Self {
        Self { claim: self.claim.clone(), metadata: self.metadata.clone() }
    }
}
impl Drop for BackendCustodySlotMemory {
    fn drop(&mut self) {
        if let Some(claim) = self.claim.take() {
            drop(Arc::into_inner(claim));
        }
    }
}

mod cleanup;
pub use cleanup::BackendCustodyCleanup;
pub(super) use cleanup::latch;
impl Drop for BackendCustodyMemory {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            // No Weak/raw projections exist. The final control allocation is
            // freed before the returned actual claim becomes reusable.
            drop(Arc::into_inner(owner));
        }
    }
}
impl Drop for Metadata {
    fn drop(&mut self) {
        // This passive Box is charged by the same original metadata guard. Remove
        // it under Core, then free it outside Core before that last claim drops.
        let detached = self.core.upgrade().and_then(|core| {
            let mut state = crate::scope::lock(&core.state);
            state
                .jobs
                .get_mut(&self.context.job_id())
                .and_then(|record| record.custody_deadlines.take())
        });
        drop(detached);
    }
}
