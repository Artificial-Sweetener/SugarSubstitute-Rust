// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Seal close-only child proof to its own existing held cleanup transaction.

use super::{CaptureBundleOrigin, CaptureBundleRetirement, retirement::RetirementAuthority};
use crate::{
    admission::{Core, State},
    desktop_capture::control::DesktopCapturePhase,
    desktop_capture::metadata::DesktopCaptureCustodyRef,
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, AdmittedJobResources, JobContext, Moment,
};

pub(super) fn validate(
    core: &Core,
    state: &State,
    target: &CaptureBundleOrigin,
    custody: &DesktopCaptureCustodyRef,
    now: Option<Moment>,
) -> Result<Moment, AdmissionError> {
    let origin = target.state()?;
    let owner = origin.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
    let record = super::validate::association(core, state, &origin.original)?;
    if !std::ptr::eq(core, owner.as_ref())
        || !origin.desktop_child
        || !record.capture_bundle_issued
        || record.desktop_capture_custody.as_ref().is_none_or(|actual| !actual.same(custody))
        || record
            .native_custody
            .as_ref()
            .is_none_or(|actual| !Arc::ptr_eq(actual, &custody.cleanup))
        || custody.cleanup.execution != core.id
        || custody.cleanup.job != record.spec.id
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let cleanup = lock(&custody.cleanup.state);
    let stage = lock(&custody.state);
    if stage.phase != DesktopCapturePhase::Cleaning
        || stage.accepted_stage.is_some()
        || !cleanup.stopped
        || !cleanup.held()
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let cutoff = match &cleanup.snapshot {
        crate::capture_attach::control::CaptureCleanupSnapshot::Latched { budget, .. } => {
            budget.settle_until
        }
        crate::capture_attach::control::CaptureCleanupSnapshot::ClockUnavailable(cause) => {
            return Err(AdmissionError::Unexpected(crate::runtime::error(cause.clone())));
        }
        _ => return Err(AdmissionError::InvalidCaptureAdmission),
    };
    if let Some(now) = now.filter(|now| *now >= cutoff) {
        return Err(AdmissionError::DeadlineExpired { deadline: cutoff, now });
    }
    Ok(cutoff)
}

impl Core {
    /// Derive cleanup only from this actual child's existing original transaction.
    /// No new close job, claim or deadline is admitted during cancellation/shutdown.
    pub(crate) fn desktop_capture_bundle_retirement(
        &self,
        target: &CaptureBundleOrigin,
        custody: &DesktopCaptureCustodyRef,
    ) -> Result<CaptureBundleRetirement, AdmissionError> {
        {
            let state = lock(&self.state);
            validate(self, &state, target, custody, None)?;
        }
        let now = self.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let (custody, original, cutoff) = {
            let state = lock(&self.state);
            let cutoff = validate(self, &state, target, custody, Some(now))?;
            let origin = target.state()?;
            let record = super::validate::association(self, &state, &origin.original)?;
            let custody = record
                .desktop_capture_custody
                .as_ref()
                .ok_or(AdmissionError::InvalidCaptureAdmission)?
                .clone();
            let original = JobContext::new(
                self.id,
                record.scope_id,
                record.spec,
                Some(cutoff),
                record.job.cancellation(),
                self.clock.clone(),
                AdmittedJobResources::new(origin.original.memory().clone(), None),
            );
            (custody, original, cutoff)
        };
        Ok(CaptureBundleRetirement {
            authority: RetirementAuthority::Desktop { custody, original },
            origin: target.clone(),
            cutoff,
        })
    }
}
