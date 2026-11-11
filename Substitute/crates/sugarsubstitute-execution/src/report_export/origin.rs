// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain the sole original envelope until its private control and native owners die.
use super::{REPORT_EXPORT_BYTES, REPORT_EXPORT_OUTPUTS, resources::Counters};
use crate::{admission::Core, scope::lock};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext, MemoryClaim};

/// Consume a genuine once-issued export admission; there is no public constructor.
pub struct ReportExportAdmission {
    pub(super) origin: ReportExportOrigin,
}
impl ReportExportAdmission {
    /// Validate actual original controls before consuming external inputs or allocating owners.
    /// # Errors
    /// Preserves genuine main cancellation, current deadlines, clock and ledger failures.
    pub fn check_effect(&self) -> Result<(), AdmissionError> {
        self.origin.check_effect()
    }
    /// Move identical original custody without allocation or renewed authority.
    pub fn into_origin(self) -> ReportExportOrigin {
        self.origin
    }
}
/// Closed original native export custody; aliases retain the same complete envelope.
pub struct ReportExportOrigin(pub(super) Option<Arc<OriginState>>);
pub(super) struct OriginState {
    pub(super) core: Arc<Core>,
    pub(super) original: JobContext,
    pub(super) counters: Arc<Counters>,
    pub(super) _memory: MemoryClaim,
}
impl Drop for OriginState {
    fn drop(&mut self) {
        let removed = {
            let mut state = lock(&self.core.state);
            let removed = state.report_resources.remove(&self.original.job_id());
            Core::retire_scopes(&mut state);
            removed
        };
        drop(removed);
        self.core.changed();
    }
}
#[repr(C)]
struct Control<T> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: T,
}
impl Clone for ReportExportOrigin {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for ReportExportOrigin {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            drop(Arc::into_inner(value));
        }
    }
}
impl ReportExportOrigin {
    pub(super) fn state(&self) -> Result<&OriginState, AdmissionError> {
        self.0.as_deref().ok_or(AdmissionError::InvalidReportExportAdmission)
    }
    /// Compare only the identical private allocation; equality grants no native effects.
    pub fn same_origin(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    /// Check actual original Core state, clock, cancellation and current scope deadlines.
    /// # Errors
    /// Preserves genuine original control/clock failures and rejects stale custody.
    pub fn check_effect(&self) -> Result<(), AdmissionError> {
        let origin = self.state()?;
        let now = origin.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&origin.core.state);
        super::admission::effect(&origin.core, &state, &origin.original, &origin.counters, now)
    }
    /// Borrow reconstructed original Core ports, with no native ledger projection.
    /// # Errors
    /// Returns invalid admission if the closed original owner is unavailable.
    pub fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        Ok(&self.state()?.original)
    }
    /// Return the complete original fixed reservation in bytes, unchanged across aliases.
    pub const fn retained_bytes(&self) -> u64 {
        REPORT_EXPORT_BYTES
    }
    /// Count all original fixed ledger/control metadata within the 128 KiB envelope.
    pub const fn control_storage_bytes(&self) -> usize {
        Self::storage_bytes()
    }
    pub(super) const fn storage_bytes() -> usize {
        size_of::<Control<OriginState>>()
            + size_of::<Control<Counters>>()
            + size_of::<super::resources::ResourceRecord>()
    }
    /// Reserve before producing an output. Drop is passive and never closes native state.
    /// # Errors
    /// Rejects original control failures or exhaustion of the independent 68-output ledger.
    pub fn reserve_output(&self) -> Result<ReportExportOutput, AdmissionError> {
        self.check_effect()?;
        let origin = self.state()?;
        let total = origin
            .counters
            .outputs
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < REPORT_EXPORT_OUTPUTS).then_some(count + 1)
            })
            .map_err(|_| AdmissionError::ReportExportOutputCapacity)?
            + 1;
        origin.counters.high_water.fetch_max(total, Ordering::AcqRel);
        origin.core.changed();
        Ok(ReportExportOutput { origin: self.clone() })
    }
}
/// Passive native output credit retaining the same original complete byte envelope.
/// Concrete native owners drop this only after checked physical close or proven no-output.
#[must_use = "Retain output credit through actual checked native retirement"]
pub struct ReportExportOutput {
    origin: ReportExportOrigin,
}
impl Drop for ReportExportOutput {
    fn drop(&mut self) {
        if let Some(origin) = self.origin.0.as_deref() {
            origin.counters.outputs.fetch_sub(1, Ordering::AcqRel);
            origin.core.changed();
        }
    }
}
