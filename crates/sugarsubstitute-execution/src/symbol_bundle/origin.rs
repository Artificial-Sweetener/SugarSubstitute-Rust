// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain the unique selected-release claim through final private control destruction.
use crate::{admission::Core, scope::lock};
use std::sync::{Arc, Weak, atomic::AtomicUsize};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, MemoryClaim, NativeHandleClaim, NativeHandleLease,
    NativeHandleOutput,
};
pub(super) const BUNDLE_BYTES: u64 = 2_097_152;
pub(super) const METADATA_BYTES: usize = 65_536;
/// Consume one genuine selected-release admission; no public constructor exists.
pub struct SymbolBundleAdmission {
    pub(super) origin: SymbolBundleOrigin,
}
impl SymbolBundleAdmission {
    /// Move the same original2MiB claim and96 output partition without allocating.
    pub fn into_origin(self) -> SymbolBundleOrigin {
        self.origin
    }
}
/// Clone only this actual original custody. Every final control frees before its claim.
/// No raw Arc/Weak, MemoryClaim or native partition projection is available.
pub struct SymbolBundleOrigin(pub(super) Option<Arc<OriginState>>);
pub(super) struct OriginState {
    pub(super) core: Weak<Core>,
    pub(super) original: JobContext,
    pub(super) ledger: NativeHandleLease,
    pub(super) selected: NativeHandleClaim,
    pub(super) purpose: super::purpose::Purpose,
    pub(super) _memory: MemoryClaim,
}
#[repr(C)]
struct Control {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: OriginState,
}
impl Clone for SymbolBundleOrigin {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for SymbolBundleOrigin {
    /// The private no-Weak control allocation dies before moved payload/claim fields.
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            drop(Arc::into_inner(value));
        }
    }
}
impl SymbolBundleOrigin {
    pub(super) fn state(&self) -> Result<&OriginState, AdmissionError> {
        self.0.as_deref().ok_or(AdmissionError::InvalidSymbolAdmission)
    }
    /// Compare only the identical private original allocation, without native authority.
    pub fn same_origin(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    /// Validate real original record/ledger and controls after sampling outside locks.
    /// # Errors
    /// Returns actual cancellation, narrowed scope cutoff, retired admission or clock cause.
    pub fn check_effect(&self) -> Result<(), AdmissionError> {
        let origin = self.state()?;
        let core = origin.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&core.state);
        super::admission::effect(
            &core,
            &state,
            &origin.original,
            &origin.ledger,
            now,
            true,
            origin.purpose,
        )
    }
    /// Borrow reconstructed actual Core clock/cancellation/memory ports.
    /// Native selected accounting remains sealed here rather than projected into this context.
    /// # Errors
    /// Returns invalid admission if the private owner is unavailable.
    pub fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        Ok(&self.state()?.original)
    }
    /// Count the single actual private Arc control inside existing selected metadata.
    pub const fn control_storage_bytes(&self) -> usize {
        size_of::<Control>()
    }
    pub(super) const fn layout_fits() -> bool {
        size_of::<Control>() <= METADATA_BYTES
    }
    /// Return the original selected-release claim in bytes; the fact grants no authority.
    pub const fn retained_bytes(&self) -> u64 {
        BUNDLE_BYTES
    }
    /// Reserve one actual selected96 output after real current original admission validation.
    /// This is passive accounting; native consumers still check every actual OS effect.
    /// # Errors
    /// Preserves actual control failures and the genuine fixed partition capacity error.
    pub fn reserve_output(&self) -> Result<NativeHandleOutput, AdmissionError> {
        self.check_effect()?;
        self.state()?.selected.reserve_output().map_err(AdmissionError::NativeResources)
    }
}
