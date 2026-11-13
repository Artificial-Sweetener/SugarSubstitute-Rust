// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Destroy retained data and final control blocks before the single original claim.

use super::{core_binding::OriginalPreparation, slot::PreparationSlot, state::NativeCapture};
use crate::{admission::Core, scope::lock};
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(any(windows, test))]
use sugarsubstitute_contracts::execution::MemoryClaim;

pub(super) struct NativeOwnership {
    pub(super) capture: Option<Box<NativeCapture>>,
    pub(super) borrowed: bool,
    pub(super) callbacks: u16,
    #[cfg(any(windows, test))]
    pub(super) acquisition_open: bool,
}
pub(super) struct PreparationPayload {
    pub(super) sources: super::source_cache::PreparationSources,
    pub(super) native: Mutex<NativeOwnership>,
    pub(super) native_failure: OnceLock<super::environment::NativeFailure>,
    pub(super) slot: OnceLock<Arc<PreparationSlot>>,
    pub(super) binding: OriginalPreparation,
    pub(super) core: Arc<Core>,
    #[cfg(any(windows, test))]
    memory: MemoryClaim,
}
impl PreparationPayload {
    #[cfg(any(windows, test))]
    pub(super) fn new(core: Arc<Core>, binding: OriginalPreparation, memory: MemoryClaim) -> Self {
        Self {
            sources: super::source_cache::PreparationSources::new(),
            native: Mutex::new(NativeOwnership {
                capture: None,
                borrowed: false,
                callbacks: 0,
                acquisition_open: true,
            }),
            native_failure: OnceLock::new(),
            slot: OnceLock::new(),
            binding,
            core,
            memory,
        }
    }
    #[cfg(any(windows, test))]
    pub(super) fn claimed_bytes(&self) -> u64 {
        self.memory.bytes()
    }
    #[cfg(any(windows, test))]
    pub(super) fn publish_quiescence(&self) {
        let native = lock(&self.native);
        if let Some(slot) = self.slot.get() {
            lock(&slot.state).callback_quiescent = native.callbacks == 0 && !native.borrowed;
        }
    }
    fn settled(&self) -> bool {
        let native = lock(&self.native);
        !native.borrowed
            && native.callbacks == 0
            && native.capture.as_ref().is_none_or(|capture| capture.native_settled)
    }
}
impl Drop for PreparationPayload {
    fn drop(&mut self) {
        if let Some(slot) = self.slot.get() {
            self.binding.detach(slot);
        }
        // Declaration order then destroys native data, passive slot, reconstructed
        // control and Core alias before the one original full claim.
    }
}

/// This closed guard exposes neither an Arc projection nor a separate claim.
pub(super) struct WholePreparation(Option<Arc<PreparationPayload>>);
impl WholePreparation {
    #[cfg(any(windows, test))]
    pub(super) fn new(payload: PreparationPayload) -> Self {
        Self(Some(Arc::new(payload)))
    }
    pub(super) fn payload(&self) -> Option<&PreparationPayload> {
        self.0.as_deref()
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}
impl Clone for WholePreparation {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for WholePreparation {
    fn drop(&mut self) {
        if let Some(shared) = self.0.take() {
            // Consuming into_inner removes the final Arc allocation before the
            // returned payload tears down its data and original claim.
            if let Some(payload) = Arc::into_inner(shared) {
                if payload.settled() {
                    drop(payload);
                } else {
                    // A failed native handoff/last abandoned alias must retain
                    // the complete original custody and truthful held accounting.
                    // Failed Free cannot be retried by a final teardown callback.
                    std::mem::forget(payload);
                }
            }
        }
    }
}
