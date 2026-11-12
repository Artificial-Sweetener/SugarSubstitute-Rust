// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep one original bundle claim until the final private control allocation is freed.

use crate::{admission::Core, scope::lock};
use std::sync::{Arc, Mutex, Weak, atomic::AtomicUsize};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, JobSpec, MemoryClaim, Moment, ScopeId, TimeError,
};

pub(crate) const RESERVATION_BYTES: u64 = 4_194_304;
pub(crate) const BUNDLE_BYTES: u64 = 2_097_152;
pub(crate) const METADATA_BYTES: usize = 65_536;

/// Consume the sole bundle issuance from the real original execution record.
/// Construction is private to that record's admission producer.
pub struct CaptureBundleAdmission {
    pub(crate) origin: CaptureBundleOrigin,
}
impl CaptureBundleAdmission {
    /// Move original bundle custody without allocating or issuing another claim.
    pub fn into_origin(self) -> CaptureBundleOrigin {
        self.origin
    }
    /// Validate fresh effect authority through the actual original Core record.
    ///
    /// # Errors
    /// Preserves original admission, cancellation, deadline and clock failures.
    pub fn check(&self) -> Result<(), AdmissionError> {
        self.origin.check_effect()
    }
    /// Borrow reconstructed original ports; supplied matching contexts never provide ports.
    ///
    /// # Errors
    /// Rejects an unavailable private owner without manufacturing a context.
    pub fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        self.origin.original_context()
    }
}

/// Clone custody of the same actual bundle claim without exposing Arc or Weak aliases.
/// Every private alias consumes `Arc::into_inner` on drop. The last control allocation
/// is freed before returned payload fields and the final original claim are destroyed.
pub struct CaptureBundleOrigin(pub(crate) Option<Arc<OriginState>>);

pub(crate) struct OriginState {
    pub(crate) core: Weak<Core>,
    pub(crate) original: JobContext,
    pub(crate) desktop_child: bool,
    pub(crate) cleanup: Mutex<CleanupLatch>,
    pub(crate) _memory: MemoryClaim,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CleanupBinding {
    pub(crate) scope: ScopeId,
    pub(crate) spec: JobSpec,
    pub(crate) deadline: Moment,
}
pub(crate) enum CleanupLatch {
    Dormant,
    Sampling(CleanupBinding),
    Latched { binding: CleanupBinding, cutoff: Moment },
    ClockFailed,
    ArithmeticFailed(TimeError),
}

// ArcInner in pinned Rust 1.93.1 is repr(C), with these two atomic counters
// followed by T. This compiler layout includes payload alignment and padding.
#[repr(C)]
struct OriginControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: OriginState,
}

impl Clone for CaptureBundleOrigin {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for CaptureBundleOrigin {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
    }
}
impl CaptureBundleOrigin {
    pub(crate) fn state(&self) -> Result<&OriginState, AdmissionError> {
        self.0.as_deref().ok_or(AdmissionError::InvalidCaptureAdmission)
    }
    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
    /// Check the actual original live record after sampling its clock outside locks.
    ///
    /// # Errors
    /// Rejects retired, cancelled, expired or foreign actual admission; fatal clock
    /// observation preserves its original cause and requests the owner's shutdown.
    pub fn check_effect(&self) -> Result<(), AdmissionError> {
        let origin = self.state()?;
        let core = origin.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&core.state);
        super::validate::effect(&core, &state, &origin.original, now, true)
    }
    /// Borrow the actual Core clock, cancellation and original admitted memory ports.
    ///
    /// # Errors
    /// Rejects an unavailable private owner. Numeric matching facts cannot create ports.
    pub fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        Ok(&self.state()?.original)
    }
    /// Return compiled private Arc control storage, counted once in bundle metadata.
    pub const fn control_storage_bytes(&self) -> usize {
        size_of::<OriginControlLayout>()
    }
    /// Return the fixed original bundle claim in bytes; this fact grants no authority.
    pub const fn retained_bytes(&self) -> u64 {
        BUNDLE_BYTES
    }
    /// Return the actual fixed original reservation in bytes, without renewing it.
    pub const fn reservation_bytes(&self) -> u64 {
        RESERVATION_BYTES
    }
    pub(crate) const fn layout_fits() -> bool {
        size_of::<OriginControlLayout>() <= METADATA_BYTES
    }
}
