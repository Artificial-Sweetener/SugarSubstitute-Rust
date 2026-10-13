// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Match actual retained ledger and record facts without invoking supplied context ports.

use super::origin::RESERVATION_BYTES;
use crate::admission::{Core, Record, State};
use sugarsubstitute_contracts::execution::{
    AdmissionError, AdmittedJobResources, CancelControl, JobClass, JobContext, JobPhase, Moment,
    Operation,
};

pub(crate) fn association<'a>(
    core: &Core,
    state: &'a State,
    observed: &JobContext,
) -> Result<&'a Record, AdmissionError> {
    let record =
        state.jobs.get(&observed.job_id()).ok_or(AdmissionError::InvalidCaptureAdmission)?;
    if observed.owner_id() != core.id
        || record.scope_id != observed.scope_id()
        || record.spec != observed.spec()
        || record.deadline != observed.deadline()
        || record.memory.as_ref().is_none_or(|memory| !memory.matches_lease(observed.memory()))
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(record)
}

fn live_facts(
    core: &Core,
    state: &State,
    observed: &JobContext,
) -> Result<Option<Moment>, AdmissionError> {
    let record = association(core, state, observed)?;
    if state.stopping || core.clock.failed() {
        return Err(AdmissionError::ShuttingDown);
    }
    if record.phase != JobPhase::Running || record.retiring {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    if record.job.cancellation().is_requested() {
        return Err(AdmissionError::ScopeClosed { scope_id: record.scope_id });
    }
    let mut cutoff = record.deadline;
    let mut current = Some(record.scope_id);
    let mut ancestors = 0;
    while let Some(id) = current {
        // Both currently supported scope partitions may occur in one genuine ancestry.
        if ancestors >= 64 + 132 {
            return Err(AdmissionError::ScopeClosed { scope_id: record.scope_id });
        }
        ancestors += 1;
        let scope = state
            .scopes
            .get(&id)
            .ok_or(AdmissionError::ScopeClosed { scope_id: record.scope_id })?;
        if scope.closed || scope.abandoned || scope.cancel.is_requested() {
            return Err(AdmissionError::ScopeClosed { scope_id: record.scope_id });
        }
        cutoff = crate::scope::earliest(cutoff, scope.deadline);
        current = scope.parent;
    }
    Ok(cutoff)
}

pub(crate) fn live(
    core: &Core,
    state: &State,
    observed: &JobContext,
    now: Moment,
) -> Result<(), AdmissionError> {
    let cutoff = live_facts(core, state, observed)?;
    if let Some(deadline) = cutoff.filter(|deadline| *deadline <= now) {
        return Err(AdmissionError::DeadlineExpired { deadline, now });
    }
    Ok(())
}

pub(crate) fn effect(
    core: &Core,
    state: &State,
    observed: &JobContext,
    now: Moment,
    issued: bool,
) -> Result<(), AdmissionError> {
    live(core, state, observed, now)?;
    let record = association(core, state, observed)?;
    if record.spec.operation != Operation::NativeLifecycle
        || record.spec.class != JobClass::InfrastructureLifecycle
        || record.spec.reservation_bytes != RESERVATION_BYTES
        || record.spec.replacement.is_some()
        || record.spec.deadline.is_none()
        || record.deadline.is_none()
        || record.capture_bundle_issued != issued
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    #[cfg(any(windows, test))]
    if let Some(custody) = &record.desktop_capture_custody {
        let cleanup = crate::scope::lock(&custody.cleanup.state);
        let stage = crate::scope::lock(&custody.state);
        if cleanup.stopped
            || matches!(
                stage.phase,
                crate::desktop_capture::control::DesktopCapturePhase::Cleaning
                    | crate::desktop_capture::control::DesktopCapturePhase::Retired
                    | crate::desktop_capture::control::DesktopCapturePhase::Transferred
                    | crate::desktop_capture::control::DesktopCapturePhase::UnissuedReturned
            )
        {
            return Err(AdmissionError::CaptureRetiring);
        }
    }
    Ok(())
}

pub(crate) fn cleanup(
    core: &Core,
    state: &State,
    observed: &JobContext,
    now: Moment,
) -> Result<(), AdmissionError> {
    live(core, state, observed, now)?;
    cleanup_facts(core, state, observed)
}

pub(crate) fn cleanup_facts(
    core: &Core,
    state: &State,
    observed: &JobContext,
) -> Result<(), AdmissionError> {
    live_facts(core, state, observed)?;
    let record = association(core, state, observed)?;
    if record.spec.operation != Operation::NativeLifecycle
        || record.spec.class != JobClass::InfrastructureLifecycle
        || record.spec.reservation_bytes < 4096
        || record.spec.replacement.is_some()
        || record.spec.deadline.is_none()
        || record.deadline.is_none()
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(())
}

pub(crate) fn reconstruct(core: &Core, record: &Record, observed: &JobContext) -> JobContext {
    JobContext::new(
        core.id,
        record.scope_id,
        record.spec,
        record.deadline,
        record.job.cancellation(),
        core.clock.clone(),
        AdmittedJobResources::new(observed.memory().clone(), None),
    )
}
