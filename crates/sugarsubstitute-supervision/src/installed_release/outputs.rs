// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the fixed capture output quota with the concrete physical owners.

use super::allocation::{
    AllocationOrigin, CapacityFailureKind, CaptureCapacityFailure, SharedCaptureLoan,
};
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicUsize, Ordering},
};

const CAPTURE_OUTPUTS: u8 = 96;

struct QuotaData {
    held: AtomicU8,
}
/// Share accounting custody without file, process or module effect authority.
pub(crate) struct CaptureOutputQuota {
    data: Option<Arc<QuotaData>>,
    _allocation: SharedCaptureLoan,
    original: AllocationOrigin,
}
#[repr(C)]
struct QuotaControl {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: QuotaData,
}
impl CaptureOutputQuota {
    /// Consume the named quota control loan before allocating its sole atomic owner.
    pub(in crate::installed_release) fn new(allocation: SharedCaptureLoan) -> Self {
        Self {
            data: Some(Arc::new(QuotaData { held: AtomicU8::new(0) })),
            original: allocation.original().clone(),
            _allocation: allocation,
        }
    }
    pub(super) const fn control_storage_bytes() -> usize {
        size_of::<QuotaControl>()
    }
    /// Reserve one possible output before a durable concrete cell invokes its producer.
    pub(super) fn reserve_output(&self) -> Result<CaptureOutputGuard, CaptureCapacityFailure> {
        let data =
            self.data.as_ref().ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Missing))?;
        data.held
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                held.checked_add(1).filter(|held| *held <= CAPTURE_OUTPUTS)
            })
            .map_err(|_| CaptureCapacityFailure::new(CapacityFailureKind::Occupied))?;
        Ok(CaptureOutputGuard { quota: self.clone() })
    }
    pub(crate) fn original(&self) -> &AllocationOrigin {
        &self.original
    }
}
impl Clone for CaptureOutputQuota {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            _allocation: self._allocation.clone(),
            original: self.original.clone(),
        }
    }
}
impl Drop for CaptureOutputQuota {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            let _completed = Arc::into_inner(data);
        }
    }
}

/// Keep one original accounting cell with its durable actual native output.
/// Its concrete file or module owner alone establishes no-output or checked closure.
pub(crate) struct CaptureOutputGuard {
    quota: CaptureOutputQuota,
}
impl CaptureOutputGuard {
    pub(crate) fn original(&self) -> &AllocationOrigin {
        self.quota.original()
    }
}
impl Drop for CaptureOutputGuard {
    fn drop(&mut self) {
        if let Some(data) = self.quota.data.as_ref()
            && data
                .held
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| held.checked_sub(1))
                .is_ok()
        {
            return;
        }
        std::mem::forget(self.quota.clone());
        tracing::error!(operation = "capture_output_retained", "Capture output custody retained");
    }
}

/// Retain one of the verified bundle's two pre-reserved module accounting cells.
/// This opaque value grants no native operation and exposes no raw handle or factory.
/// Crash keeps it with the actual module until checked no-output/closure, or forever
/// with a permanently pinned or uncertain module owner.
pub struct CaptureModuleOutput {
    _guard: CaptureOutputGuard,
    causes: crate::protected_storage::native_file::NativeCauseDestinations,
}
impl CaptureModuleOutput {
    /// Observe actual original admission before a fresh finite module effect.
    /// This never adopts or closes a caller's HMODULE.
    pub fn check_effect(&self) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        self._guard.original().original().check_effect()
    }
    /// Retain a genuine loader status in this output's preheld cause destination.
    /// The caller keeps the actual module and this whole guard on every error.
    pub fn native_failure(
        &mut self,
        operation: &'static str,
        cause: windows::core::Error,
    ) -> crate::ClaimError {
        self.causes.selected_error(
            operation,
            super::causes::CaptureBoundedCause::Windows(cause),
            false,
        )
    }

    pub(super) fn from_guard(
        guard: CaptureOutputGuard,
        causes: crate::protected_storage::native_file::NativeCauseDestinations,
    ) -> Self {
        Self { _guard: guard, causes }
    }
}
