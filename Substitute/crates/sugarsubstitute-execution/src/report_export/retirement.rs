// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Validate independent close-only cleanup against the same original retained custody.
use super::ReportExportOrigin;
use crate::{
    ExecutionClient,
    admission::{Core, State},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobClass, JobContext, Moment, Operation,
};

/// Close-only proof from the actual paired activated cleanup and immutable original cutoff.
pub struct ReportExportRetirement {
    original: JobContext,
    origin: ReportExportOrigin,
    cutoff: Moment,
}
fn validate(
    core: &Core,
    state: &State,
    origin: &ReportExportOrigin,
    observed: &JobContext,
    now: Moment,
) -> Result<Moment, AdmissionError> {
    let target = origin.state()?;
    if !std::ptr::eq(Arc::as_ptr(&target.core), core) || observed.native_handles().is_some() {
        return Err(AdmissionError::InvalidReportExportAdmission);
    }
    crate::capture_bundle::validate::live(core, state, observed, now)?;
    let record = crate::capture_bundle::validate::association(core, state, observed)?;
    let binding =
        record.report_cleanup.as_ref().ok_or(AdmissionError::InvalidReportExportAdmission)?;
    let original = binding.original.ok_or(AdmissionError::InvalidReportExportAdmission)?;
    let resource = state
        .report_resources
        .get(&target.original.job_id())
        .ok_or(AdmissionError::InvalidReportExportAdmission)?;
    if record.spec.operation != Operation::Infrastructure
        || record.spec.class != JobClass::InfrastructureLifecycle
        || record.spec.reservation_bytes != super::REPORT_EXPORT_CLEANUP_BYTES
        || record.spec.replacement.is_some()
        || !binding.paired
        || binding.main_spec != target.original.spec()
        || binding.main_scope.id != target.original.scope_id()
        || !original.matches_actual(core.id, record.scope_id, record.spec.id)
        || record.deadline != Some(original.deadline())
        || !resource.issued
        || resource.scope_id != target.original.scope_id()
        || !resource.matches(&target.counters)
        || state.jobs.get(&binding.main_spec.id).is_some_and(|main| {
            !matches!(main.phase, sugarsubstitute_contracts::execution::JobPhase::Terminal(_))
                || main.retiring
        })
    {
        return Err(AdmissionError::InvalidReportExportAdmission);
    }
    if now >= original.deadline() {
        return Err(AdmissionError::DeadlineExpired { deadline: original.deadline(), now });
    }
    Ok(original.deadline())
}
impl ExecutionClient {
    /// Derive close-only proof from the actual independent activated pre-held cleanup job.
    /// Target main may be terminal or acknowledged; every output retains its original claim.
    /// # Errors
    /// Rejects foreign origins, unrelated/unactivated cleanup, mismatched ledgers and
    /// genuine original cleanup clock/cancellation/deadline failures.
    pub fn report_export_retirement(
        &self,
        target: &ReportExportOrigin,
        observed: &JobContext,
    ) -> Result<ReportExportRetirement, AdmissionError> {
        let origin = target.state()?;
        if !Arc::ptr_eq(&origin.core, &self.core) {
            return Err(AdmissionError::InvalidReportExportAdmission);
        }
        {
            let state = lock(&self.core.state);
            crate::capture_bundle::validate::association(&self.core, &state, observed)?;
        }
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let original;
        let cutoff;
        {
            let state = lock(&self.core.state);
            cutoff = validate(&self.core, &state, target, observed, now)?;
            let record =
                crate::capture_bundle::validate::association(&self.core, &state, observed)?;
            original = crate::capture_bundle::validate::reconstruct(&self.core, record, observed);
        }
        Ok(ReportExportRetirement { original, origin: target.clone(), cutoff })
    }
}
impl ReportExportRetirement {
    /// Recheck genuine activated cleanup and its immutable first cutoff before each close.
    /// # Errors
    /// Preserves actual cleanup cancellation, deadline and original clock causes.
    pub fn check(&self) -> Result<(), AdmissionError> {
        let core = &self.origin.state()?.core;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&core.state);
        if validate(core, &state, &self.origin, &self.original, now)? == self.cutoff {
            Ok(())
        } else {
            Err(AdmissionError::InvalidReportExportAdmission)
        }
    }
    /// Borrow the genuine reconstructed cleanup context; it grants no fresh main effects.
    pub const fn original_context(&self) -> &JobContext {
        &self.original
    }
    /// Return the first immutable <=15-second cleanup cutoff in its original clock epoch.
    pub const fn cleanup_cutoff(&self) -> Moment {
        self.cutoff
    }
    /// Compare only identical original custody without asserting physical retirement.
    pub fn matches_origin(&self, target: &ReportExportOrigin) -> bool {
        self.origin.same_origin(target)
    }
}
