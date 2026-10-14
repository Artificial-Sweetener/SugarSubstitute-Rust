// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the original capture record inside the central admission transaction.
use crate::{
    admission::{Core, State},
    desktop_capture::{control::DesktopCapturePhase, metadata::DesktopCaptureCustodyRef},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{execution::*, lifecycle::RunKey};

/// Read exact owner facts without a supplied provider, allocation or native call.
pub(crate) fn source(
    core: &Core,
    state: &State,
    scope: ScopeId,
    custody: &DesktopCaptureCustodyRef,
    run: RunKey,
    now: Moment,
) -> Result<(), AdmissionError> {
    if state.stopping || core.clock.failed() {
        return Err(AdmissionError::ShuttingDown);
    }
    let record =
        state.jobs.get(&custody.cleanup.job).ok_or(AdmissionError::InvalidCaptureAdmission)?;
    if custody.cleanup.execution != core.id
        || custody.cleanup.full_target.desktop() != Some(run)
        || record.scope_id != scope
        || record.retiring
        || !record.terminal_published
        || record.phase != JobPhase::Terminal(TerminalKind::Success)
        || !record.capture_bundle_issued
        || record.desktop_capture_custody.as_ref().is_none_or(|actual| !actual.same(custody))
        || record
            .native_custody
            .as_ref()
            .is_none_or(|actual| !Arc::ptr_eq(actual, &custody.cleanup))
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let until = crate::admission_liveness::scope_ancestry_deadline(core, state, record)?
        .map_or(custody.cleanup.attach_until, |until| until.min(custody.cleanup.attach_until));
    if now >= until {
        return Err(AdmissionError::DeadlineExpired { deadline: until, now });
    }
    let cleanup = lock(&custody.cleanup.state);
    let stage = lock(&custody.state);
    if cleanup.stopped
        || !stage.registered_published
        || stage.phase != DesktopCapturePhase::Registered
        || stage.accepted_cleanup.is_some()
    {
        return Err(AdmissionError::CaptureRetiring);
    }
    if stage.authenticated_creator.is_none_or(|creator| !creator.matches(run)) {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(())
}
/// Reject before charging central capacity or consuming the source's once-only flag.
pub(crate) fn validate(
    core: &Core,
    state: &State,
    scope: ScopeId,
    spec: JobSpec,
    custody: &DesktopCaptureCustodyRef,
    run: RunKey,
    now: Moment,
) -> Result<(), AdmissionError> {
    if spec.operation != Operation::NativeLifecycle
        || spec.class != JobClass::InfrastructureLifecycle
        || spec.reservation_bytes != 4096
        || spec.deadline != Some(custody.cleanup.attach_until)
        || spec.replacement.is_some()
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    source(core, state, scope, custody, run, now)?;
    let stage = lock(&custody.state);
    if stage.fault_job.is_some() || stage.accepted_stage.is_some() {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(())
}
