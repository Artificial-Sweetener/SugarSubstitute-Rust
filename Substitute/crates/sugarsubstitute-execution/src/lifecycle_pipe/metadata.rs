// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain native metadata outside every covered control allocation and owner box.

use super::{LifecyclePipeFailure, failure};
use crate::scope::lock;
#[cfg(windows)]
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Private aliases share one claim, with no exposed Arc or Weak constructor.
/// Every holder places this guard after the allocations it covers.
#[derive(Clone)]
pub(crate) struct Metadata(Option<Arc<State>>);

struct State {
    claim: Mutex<Option<MemoryClaim>>,
}

// Pinned Rust 1.93.1 ArcInner is repr(C): two atomic counters, then its payload.
#[repr(C)]
#[cfg(windows)]
struct ControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: State,
}

impl Metadata {
    /// Create the bounded control while the real admission owns the original reservation.
    /// Install its original claim before allocating native storage or issuing effects.
    pub(crate) fn new(claim: Option<MemoryClaim>) -> Self {
        Self(Some(Arc::new(State { claim: Mutex::new(claim) })))
    }

    /// Observe only installed original capacity; no provider or native operation runs.
    pub(crate) fn installed(&self) -> bool {
        self.0.as_deref().is_some_and(|state| lock(&state.claim).is_some())
    }

    /// Install the original nonclone claim exactly once; failure returns its capacity.
    pub(crate) fn install(&self, claim: MemoryClaim) -> Result<(), LifecyclePipeFailure> {
        let state = self.0.as_deref().ok_or_else(|| failure("pipe_metadata_owner"))?;
        let mut slot = lock(&state.claim);
        if slot.is_some() {
            return Err(failure("pipe_metadata_already_installed"));
        }
        *slot = Some(claim);
        Ok(())
    }

    /// Return actual compiled control size for preflight, including both Arc counters.
    #[cfg(windows)]
    pub(crate) const fn control_storage_bytes() -> usize {
        size_of::<ControlLayout>()
    }
}

impl Drop for Metadata {
    /// Free the last private control allocation before returning its original payload claim.
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
    }
}
