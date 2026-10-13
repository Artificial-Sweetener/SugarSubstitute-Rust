// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exactly the child completion Event and registered Wait output credits.
use crate::{admission::Core, capture_bundle::CaptureBundleOrigin};
use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Weak,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{AdmissionError, MemoryClaim};

/// Fixed original native/input partition subdivision, not an additional reservation.
pub(crate) const EVENT_METADATA_BYTES: u64 = 4096;
/// Remaining original native/input storage after its two-output accounting control.
pub const DESKTOP_CAPTURE_NATIVE_WORKSPACE_BYTES: u64 = 966_656 - EVENT_METADATA_BYTES;

/// Clone only the exact sealed two-output child accounting allocation.
/// No public capacity constructor, raw handle or Weak counter projection exists.
#[derive(Clone)]
pub(crate) struct CaptureEventClaim {
    owner: CounterRef,
}
#[derive(Clone)]
struct CounterRef {
    value: Arc<Counter>,
    _charge: Charge,
}
struct Counter {
    original: CaptureBundleOrigin,
    core: Weak<Core>,
    outputs: AtomicU8,
    high_water: AtomicU8,
}
#[derive(Clone)]
struct Charge(Option<Arc<MemoryClaim>>);
impl Drop for Charge {
    /// Free the private last charge control before releasing its original claim.
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
    }
}
#[repr(C)]
struct CounterLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Counter,
}
#[repr(C)]
struct ChargeLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: MemoryClaim,
}

/// Capacity exhaustion precedes effects and preserves the original two credits.
#[derive(Debug)]
pub(crate) struct CaptureEventCapacity;
impl fmt::Display for CaptureEventCapacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original child Event/Wait output capacity exhausted")
    }
}
impl Error for CaptureEventCapacity {}

impl CaptureEventClaim {
    /// Install only from the actual Core producer after record/custody validation.
    /// Its once-issued4096 claim is part of the original966656 native/input bytes.
    pub(crate) fn new(
        original: CaptureBundleOrigin,
        core: Weak<Core>,
        metadata: MemoryClaim,
    ) -> Self {
        let charge = Charge(Some(Arc::new(metadata)));
        Self {
            owner: CounterRef {
                value: Arc::new(Counter {
                    original,
                    core,
                    outputs: AtomicU8::new(0),
                    high_water: AtomicU8::new(0),
                }),
                _charge: charge,
            },
        }
    }
    /// Report actual closed control/value storage before the producer allocates.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<CounterLayout>()
            + size_of::<ChargeLayout>()
            + 2 * size_of::<CaptureEventOutput>()
            + size_of::<Self>()
    }
    /// Return a passive allocation identity for the actual original Core record.
    /// The private producer, sticky issuance and actual origin checks supply authority.
    pub(crate) fn identity(&self) -> usize {
        Arc::as_ptr(&self.owner.value) as usize
    }
    /// Borrow only the original sealed Core target for cleanup-proof comparison.
    pub(crate) fn original(&self) -> &CaptureBundleOrigin {
        &self.owner.value.original
    }
    /// Check the original sealed actual child record before every fresh native effect.
    pub(crate) fn check(&self) -> Result<(), AdmissionError> {
        let state = &self.owner.value;
        let core = state.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        core.check_capture_event(&state.original, self.identity())
    }
    /// Observe inert original counters; zero grants no checked-retirement proof.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> (u8, u8) {
        let state = &self.owner.value;
        (state.outputs.load(Ordering::Acquire), state.high_water.load(Ordering::Acquire))
    }
    /// Reserve one actual Event or Wait output before its native call.
    pub(crate) fn reserve_output(&self) -> Result<CaptureEventOutput, CaptureEventCapacity> {
        let state = &self.owner.value;
        let count = state
            .outputs
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < 2).then_some(count + 1)
            })
            .map_err(|_| CaptureEventCapacity)?
            + 1;
        state.high_water.fetch_max(count, Ordering::AcqRel);
        if let Some(core) = state.core.upgrade() {
            core.changed();
        }
        Ok(CaptureEventOutput { owner: self.owner.clone() })
    }
}
/// Retain one concrete produced/partial output until its owner proves checked closure.
/// Drop updates accounting only; it never closes or inspects an OS object.
pub(crate) struct CaptureEventOutput {
    owner: CounterRef,
}
impl Drop for CaptureEventOutput {
    /// Return only this output credit while outside metadata remains retained.
    fn drop(&mut self) {
        self.owner.value.outputs.fetch_sub(1, Ordering::AcqRel);
        if let Some(core) = self.owner.value.core.upgrade() {
            core.changed();
        }
    }
}
