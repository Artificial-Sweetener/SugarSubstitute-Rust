// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supervise actual registered custody after its finite registration job succeeded.
use super::{control::DesktopCapturePhase, work::DesktopCaptureWork};
use crate::{
    admission::Core,
    scope::lock,
    task::{ErasedJob, Stop},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, JobPhase, Moment, TerminalKind,
};

/// A successful registration ends only its finite job cutoff. The same actual
/// scope/owner and native custody continue until cancellation or explicit cleanup.
/// The Core->cleanup->stage fence observes a recorded cleanup request even before
/// the cleanup claimant sets `cleanup.stopped`, or after the caller's
/// earlier live observation became stale. Only the same registered owner and its
/// original stop reason qualify. This grants no effects or changed context.
pub(super) fn stop(
    core: &Core,
    work: &DesktopCaptureWork,
    context: &JobContext,
    observed: Moment,
) -> Result<Option<Stop>, AdmissionError> {
    let job: Arc<dyn ErasedJob> = work.job.clone();
    let state = lock(&core.state);
    let record = crate::capture_bundle::validate::association(core, &state, context)?;
    if !Arc::ptr_eq(&record.job, &job)
        || !record.terminal_published
        || record.phase != JobPhase::Terminal(TerminalKind::Success)
        || record.retiring
        || !record.capture_bundle_issued
        || record.desktop_capture_custody.as_ref().is_none_or(|actual| !actual.same(&work.shared))
        || record
            .native_custody
            .as_ref()
            .is_none_or(|actual| !Arc::ptr_eq(actual, &work.shared.cleanup))
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    {
        let _cleanup = lock(&work.shared.cleanup.state);
        let stage = lock(&work.shared.state);
        if !stage.registered_published {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        match (stage.phase, stage.stopped_outcome) {
            (DesktopCapturePhase::Cleaning, Some(reason)) => return Ok(Some(reason)),
            (DesktopCapturePhase::Registered, None) => {}
            _ => return Err(AdmissionError::InvalidCaptureAdmission),
        }
    }
    match crate::admission_liveness::scope_ancestry_deadline(core, &state, record) {
        Ok(Some(until)) if observed >= until => Ok(Some(Stop::TimedOut)),
        Ok(_) => Ok(None),
        Err(AdmissionError::ScopeClosed { .. } | AdmissionError::ShuttingDown) => {
            Ok(Some(Stop::Cancelled))
        }
        Err(cause) => Err(cause),
    }
}
