// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Free original control storage before releasing its one family claim.
use super::source::Source;
use std::sync::{Arc, Weak};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, JobPhase, MemoryClaim, Moment,
};

/// Sealed original creation storage; aliases retain the identical fixed claim.
/// No raw claim, Arc, native handle or caller-built context is projected.
pub struct ProcessCreationMemory(Option<Arc<Memory>>);
struct Memory {
    source: Source,
    context: JobContext,
    core: Weak<crate::admission::Core>,
    cleanup: std::sync::Mutex<Cleanup>,
    #[cfg(windows)]
    section_outputs: std::sync::atomic::AtomicUsize,
    #[cfg(windows)]
    section_issued: std::sync::atomic::AtomicBool,
    _claim: MemoryClaim,
}
impl ProcessCreationMemory {
    pub(super) fn new(
        source: Source,
        context: JobContext,
        core: Weak<crate::admission::Core>,
        claim: MemoryClaim,
    ) -> Self {
        Self(Some(Arc::new(Memory {
            source,
            context,
            core,
            cleanup: std::sync::Mutex::new(Cleanup::Fresh),
            #[cfg(windows)]
            section_outputs: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(windows)]
            section_issued: std::sync::atomic::AtomicBool::new(false),
            _claim: claim,
        })))
    }
    /// Actual compiled producer bytes, before any family/driver allocation.
    pub const fn control_storage_bytes() -> usize {
        size_of::<Memory>() + 2 * size_of::<usize>()
    }
    /// Admit only the one combined section's three distinct native output slots.
    /// The real creation owner still holds its original family and transient claims.
    #[cfg(windows)]
    pub(crate) fn reserve_section_output(&self) -> Result<SectionOutput, AdmissionError> {
        self.check_effect()?;
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        owner
            .section_outputs
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |old| (old < 3).then_some(old + 1),
            )
            .map_err(|_| AdmissionError::InvalidCaptureAdmission)?;
        Ok(SectionOutput { _memory: self.clone() })
    }
    /// Close duplicate construction before allocating the one native section owner.
    #[cfg(windows)]
    pub(crate) fn claim_recovery_section(
        &self,
        source: &crate::recovery_supervisor::startup::SourceRef,
    ) -> Result<(), AdmissionError> {
        self.check_effect()?;
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        if !matches!(&owner.source, Source::Recovery(actual) if actual.same_allocation(source)) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        owner
            .section_issued
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .map_err(|_| AdmissionError::InvalidCaptureAdmission)?;
        Ok(())
    }
    /// Match the exact process-local authenticated source before native resume admission.
    pub(crate) fn check_recovery_source(
        &self,
        source: &crate::recovery_supervisor::startup::SourceRef,
    ) -> Result<(), AdmissionError> {
        self.check_effect()?;
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        if !matches!(&owner.source, Source::Recovery(actual) if actual.same_allocation(source)) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(())
    }
    /// Deliver one coalesced application progress wake from this actual running creation.
    /// This publishes no terminal notice and grants no scheduler or runtime access.
    pub(super) fn application_observed(&self) -> Result<(), AdmissionError> {
        self.check_effect()?;
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        if core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
        Ok(())
    }
    /// Return the fixed original accounting partition without granting capacity.
    pub const fn bytes(&self) -> u64 {
        super::PROCESS_CREATION_STORAGE_BYTES
    }
    /// Compare actual original storage allocations without providers.
    pub fn same_allocation(&self, other: &Self) -> bool {
        self.0.as_ref().zip(other.0.as_ref()).is_some_and(|(a, b)| Arc::ptr_eq(a, b))
    }
    /// Borrow the genuine original context, reconstructed by this execution owner.
    /// # Errors
    /// Rejects a consumed internal owner.
    pub fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        self.0.as_ref().map(|owner| &owner.context).ok_or(AdmissionError::ShuttingDown)
    }
    /// Observe only the genuine original running record for close-only custody.
    pub(super) fn observe_retirement(&self) -> Result<Moment, AdmissionError> {
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = crate::scope::lock(&core.state);
        let record = state.jobs.get(&owner.context.job_id()).ok_or(AdmissionError::ShuttingDown)?;
        if record.phase != JobPhase::Running
            || !record.process_creation
            || record
                .memory
                .as_ref()
                .is_none_or(|memory| !memory.matches_lease(owner.context.memory()))
        {
            return Err(AdmissionError::ShuttingDown);
        }
        Ok(now)
    }
    /// Latch the same first15-second owned-output cleanup and original timer slots.
    /// Cancellation closes effects while this proof permits only existing output release.
    /// # Errors
    /// Retains arithmetic/provider failure; repeated calls cannot renew cleanup.
    pub fn begin_retirement(&self) -> Result<super::ProcessCreationRetirement, AdmissionError> {
        let now = self.observe_retirement()?;
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let budget = {
            let mut cleanup = crate::scope::lock(&owner.cleanup);
            match *cleanup {
                Cleanup::Latched(budget) => budget,
                Cleanup::Failed(cause) => return Err(cleanup_error(cause)),
                Cleanup::Fresh => {
                    match sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(now) {
                        Ok(budget) => {
                            *cleanup = Cleanup::Latched(budget);
                            budget
                        }
                        Err(cause) => {
                            *cleanup = Cleanup::Failed(cause);
                            return Err(cleanup_error(cause));
                        }
                    }
                }
            }
        };
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        {
            let mut state = crate::scope::lock(&core.state);
            let record =
                state.jobs.get_mut(&owner.context.job_id()).ok_or(AdmissionError::ShuttingDown)?;
            if !record.process_creation
                || record.phase != JobPhase::Running
                || record
                    .memory
                    .as_ref()
                    .is_none_or(|memory| !memory.matches_lease(owner.context.memory()))
            {
                return Err(AdmissionError::ShuttingDown);
            }
            let slots = record.native_deadlines.as_mut().ok_or(AdmissionError::ShuttingDown)?;
            slots.stop();
            slots.latch_capture_cleanup(budget);
        }
        core.changed();
        Ok(super::ProcessCreationRetirement { memory: self.clone(), until: budget.settle_until })
    }
    /// Check the actual running creation record, original lease, scopes and source.
    /// Every provider is sampled before Core or any owned mutex is acquired.
    /// # Errors
    /// Preserves genuine provider, cancellation, deadline or source rejection.
    pub fn check_effect(&self) -> Result<Moment, AdmissionError> {
        let owner = self.0.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let core = owner.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = crate::scope::lock(&core.state);
        let record = state.jobs.get(&owner.context.job_id()).ok_or(AdmissionError::ShuttingDown)?;
        if record.phase != JobPhase::Running
            || record.retiring
            || record.scope_id != owner.context.scope_id()
            || record
                .memory
                .as_ref()
                .is_none_or(|memory| !memory.matches_lease(owner.context.memory()))
        {
            return Err(AdmissionError::ShuttingDown);
        }
        let until = crate::admission_liveness::scope_deadline(&core, &state, record)?;
        if let Some(deadline) = until.filter(|until| *until <= now) {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
        owner.source.check_locked(&core, &state, now)?;
        Ok(now)
    }
}
impl Clone for ProcessCreationMemory {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for ProcessCreationMemory {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
    }
}

/// First cleanup transaction or its permanent original arithmetic failure.
#[derive(Clone, Copy)]
enum Cleanup {
    Fresh,
    Latched(sugarsubstitute_contracts::lifecycle::ShutdownBudget),
    Failed(sugarsubstitute_contracts::lifecycle::LifecycleRejection),
}
/// Preserve the actual original cutoff-construction cause through the typed admission boundary.
fn cleanup_error(
    cause: sugarsubstitute_contracts::lifecycle::LifecycleRejection,
) -> AdmissionError {
    AdmissionError::Unexpected(sugarsubstitute_contracts::execution::ExecutionError::new(
        sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
        Box::new(cause),
    ))
}

/// A produced section/token output retains the exact creation admission through checked close.
/// The counter bounds distinct admissions, rather than allowing slot reuse to hide outputs.
#[cfg(windows)]
pub(crate) struct SectionOutput {
    _memory: ProcessCreationMemory,
}
