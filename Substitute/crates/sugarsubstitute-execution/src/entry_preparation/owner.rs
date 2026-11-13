// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share one genuine original whole owner without exposing native or ledger projections.

use super::{
    EntryPreparationInputError,
    core_binding::OriginalPreparation,
    custody::{PreparationPayload, WholePreparation},
    slot::{PreparationPhase, PreparationSlot},
};
use crate::{admission::Core, scope::lock};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, Moment, PanicCause};

#[derive(Clone)]
pub(crate) struct EntryPreparationOwner {
    context: Arc<JobContext>,
    #[cfg(any(windows, test))]
    operation: Arc<PreparedOperationSlot>,
    whole: WholePreparation,
}
#[cfg(any(windows, test))]
pub(super) struct PreparedOperationSlot {
    sources: std::sync::OnceLock<crate::coordinator_entry::prepared_failure::ClosedPreparedSources>,
}
#[cfg(any(windows, test))]
impl PreparedOperationSlot {
    fn new() -> Self {
        Self { sources: std::sync::OnceLock::new() }
    }
}

impl EntryPreparationOwner {
    pub(super) fn new(
        core: Arc<Core>,
        binding: OriginalPreparation,
        memory: MemoryClaim,
    ) -> Result<Self, (EntryPreparationInputError, MemoryClaim)> {
        if super::layout::retained_native_bound().is_none_or(|bytes| bytes > memory.bytes()) {
            return Err((EntryPreparationInputError::Closed, memory));
        }
        let context = binding.context().clone();
        let payload = PreparationPayload::new(core, binding, memory);
        let whole = WholePreparation::new(payload);
        let owner = Self {
            context: Arc::new(context),
            #[cfg(any(windows, test))]
            operation: Arc::new(PreparedOperationSlot::new()),
            whole,
        };
        let slot = Arc::new(PreparationSlot::new(
            owner.context.owner_id(),
            owner.context.scope_id(),
            owner.context.job_id(),
        ));
        if let Some(payload) = owner.whole.payload() {
            // This fresh OnceLock has not escaped; setting it allocates nothing.
            let duplicate = payload.slot.set(slot);
            drop(duplicate);
        }
        Ok(owner)
    }

    /// Register only after the runner retains this whole owner outside provider unwind.
    pub(super) fn register(&self) -> Result<(), EntryPreparationInputError> {
        let payload = self.payload()?;
        let slot = self.passive_slot()?.clone();
        match payload.binding.register(slot) {
            Ok(()) => Ok(()),
            Err((cause, slot)) => {
                drop(slot);
                Err(cause)
            }
        }
    }

    pub(super) fn payload(&self) -> Result<&PreparationPayload, EntryPreparationInputError> {
        self.whole.payload().ok_or(EntryPreparationInputError::Closed)
    }
    pub(crate) fn original_context(&self) -> &JobContext {
        &self.context
    }
    pub(crate) fn original_binding(
        &self,
    ) -> Result<&OriginalPreparation, EntryPreparationInputError> {
        Ok(&self.payload()?.binding)
    }
    pub(crate) fn passive_slot(&self) -> Result<&Arc<PreparationSlot>, EntryPreparationInputError> {
        self.payload()?.slot.get().ok_or(EntryPreparationInputError::Closed)
    }
    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        self.whole.same(&other.whole)
    }
    #[cfg(any(windows, test))]
    pub(crate) fn operation_sources(
        &self,
    ) -> &std::sync::OnceLock<crate::coordinator_entry::prepared_failure::ClosedPreparedSources>
    {
        &self.operation.sources
    }
    pub(super) fn check_publication(&self) -> Result<Moment, EntryPreparationInputError> {
        let payload = self.payload()?;
        self.checked(|| payload.binding.check_publication(&payload.core, payload.binding.setup()))
    }
    pub(super) fn observe_control(&self) -> Result<Moment, EntryPreparationInputError> {
        let payload = self.payload()?;
        self.checked(|| payload.binding.check(&payload.core, payload.binding.setup()))
    }
    pub(crate) fn checked<T>(
        &self,
        action: impl FnOnce() -> Result<T, EntryPreparationInputError>,
    ) -> Result<T, EntryPreparationInputError> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
            Ok(outcome) => outcome,
            Err(payload) => {
                self.close_publication();
                Err(EntryPreparationInputError::Control(
                    super::EntryPreparationControlCause::ProviderPanic(crate::task::panic_cause(
                        payload,
                    )),
                ))
            }
        }
    }
    pub(crate) fn close_publication(&self) {
        if let Ok(payload) = self.payload() {
            payload.binding.close_publication();
        }
    }
    pub(super) fn close_acquisition(&self) {
        if let Ok(payload) = self.payload() {
            lock(&payload.native).acquisition_open = false;
        }
    }
    pub(super) fn callback(&self) -> Result<PreparationCallback, EntryPreparationInputError> {
        let payload = self.payload()?;
        {
            let mut native = lock(&payload.native);
            native.callbacks =
                native.callbacks.checked_add(1).ok_or(EntryPreparationInputError::Closed)?;
        }
        payload.publish_quiescence();
        Ok(PreparationCallback { owner: self.clone() })
    }
    pub(super) fn claimed_bytes(&self) -> Result<u64, EntryPreparationInputError> {
        Ok(self.payload()?.claimed_bytes())
    }
    pub(super) fn publish_phase(&self, phase: PreparationPhase) {
        if let Ok(slot) = self.passive_slot() {
            lock(&slot.state).phase = phase;
        }
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        self.close_publication();
        if let Ok(payload) = self.payload()
            && let Some(capture) = &mut lock(&payload.native).capture
        {
            capture.record(super::cause::EntryEnvironmentCause::ProviderPanic {
                source: cause.clone(),
            });
        }
    }
}

/// Retain the actual owner across callbacks and publish quiescence only after return.
pub(super) struct PreparationCallback {
    owner: EntryPreparationOwner,
}
impl Drop for PreparationCallback {
    fn drop(&mut self) {
        if let Ok(payload) = self.owner.payload() {
            {
                let mut native = lock(&payload.native);
                if native.callbacks > 0 {
                    native.callbacks -= 1;
                } else {
                    // An invalid callback count must never grant quiescence.
                    native.borrowed = true;
                }
            }
            payload.publish_quiescence();
        }
    }
}
