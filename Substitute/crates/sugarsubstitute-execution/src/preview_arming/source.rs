// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match the actual charged capture control and permanent association inside Core fences.
use super::{PreviewArmingError, PreviewSourceLoan};
use crate::{
    admission::{Core, State},
    desktop_capture::control::DesktopCapturePhase,
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::*;

pub(super) fn core(loan: &PreviewSourceLoan) -> Result<Arc<Core>, AdmissionError> {
    loan.custody.cleanup.core.upgrade().ok_or(AdmissionError::ShuttingDown)
}
pub(super) fn now(core: &Core) -> Result<Moment, PreviewArmingError> {
    core.clock
        .metadata()
        .map_err(|cause| PreviewArmingError::Admission(AdmissionError::Unexpected(cause)))
}
/// Actual current native scope is stamped by the kernel-authenticated input producer.
/// The completed capture job cutoff is deliberately absent from these lifetime checks.
pub(super) fn current(
    core: &Core,
    state: &State,
    loan: &PreviewSourceLoan,
    now: Moment,
) -> Result<(), AdmissionError> {
    if state.stopping || core.clock.failed() {
        return Err(AdmissionError::ShuttingDown);
    }
    if now >= loan.until {
        return Err(AdmissionError::DeadlineExpired { deadline: loan.until, now });
    }
    let custody = &loan.custody;
    if !std::ptr::eq(custody.cleanup.core.as_ptr(), core)
        || custody.cleanup.execution != core.id
        || custody.cleanup.full_target.desktop() != Some(loan.run)
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let native = state
        .scopes
        .get(&loan.native_scope)
        .ok_or(AdmissionError::ScopeClosed { scope_id: loan.native_scope })?;
    if !native.native || native.closed || native.abandoned || native.cancel.is_requested() {
        return Err(AdmissionError::ScopeClosed { scope_id: loan.native_scope });
    }
    if let Some(deadline) = native.deadline.filter(|until| now >= *until) {
        return Err(AdmissionError::DeadlineExpired { deadline, now });
    }
    let ordinary = state
        .scopes
        .get(&loan.source_scope)
        .ok_or(AdmissionError::ScopeClosed { scope_id: loan.source_scope })?;
    if ordinary.native || ordinary.closed || ordinary.abandoned || ordinary.cancel.is_requested() {
        return Err(AdmissionError::ScopeClosed { scope_id: loan.source_scope });
    }
    if let Some(deadline) = ordinary.deadline.filter(|until| now >= *until) {
        return Err(AdmissionError::DeadlineExpired { deadline, now });
    }
    let record =
        state.jobs.get(&custody.cleanup.job).ok_or(AdmissionError::InvalidCaptureAdmission)?;
    if record.scope_id != loan.source_scope
        || record.retiring
        || !record.terminal_published
        || !record.capture_bundle_issued
        || record.phase != JobPhase::Terminal(TerminalKind::Success)
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let cleanup = lock(&custody.cleanup.state);
    let stage = lock(&custody.state);
    if loan.control_bytes < PreviewSourceLoan::storage_bytes()
        || loan.control_bytes != stage.preview_control_bytes
        || !stage.registered_published
        || stage.accepted_cleanup.is_some()
        || stage.accepted_stage.is_some()
        || stage.authenticated_creator.is_none_or(|creator| {
            !creator.matches(loan.run)
                || !creator.matches_native(core, loan.native_scope)
                || !creator.permits_campaign(loan.counter)
        })
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    match stage.phase {
        DesktopCapturePhase::Registered
            if !cleanup.stopped
                && record
                    .desktop_capture_custody
                    .as_ref()
                    .is_some_and(|actual| actual.same(custody))
                && record
                    .native_custody
                    .as_ref()
                    .is_some_and(|actual| Arc::ptr_eq(actual, &custody.cleanup)) =>
        {
            if let Some(deadline) =
                crate::admission_liveness::scope_ancestry_deadline(core, state, record)?
                    .filter(|until| now >= *until)
            {
                return Err(AdmissionError::DeadlineExpired { deadline, now });
            }
        }
        DesktopCapturePhase::Transferred
            if stage.permanent_identity.is_some_and(|identity| {
                record.process_capture.as_ref().is_some_and(|actual| {
                    actual.full_target.desktop() == Some(loan.run)
                        && actual.capsule_identity == identity
                })
            }) && !cleanup.active
                && !cleanup.native_retired
                && !cleanup.source_retired => {}
        _ => return Err(AdmissionError::CaptureRetiring),
    }
    Ok(())
}
pub(super) fn worker(
    core: &Core,
    state: &State,
    loan: &PreviewSourceLoan,
    context: &JobContext,
    now: Moment,
) -> Result<(), AdmissionError> {
    current(core, state, loan, now)?;
    crate::capture_bundle::validate::live(core, state, context, now)?;
    if !lock(&loan.custody.state).preview_loan_issued {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let spec = context.spec();
    if context.native_handles().is_some() {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    if context.scope_id() != loan.source_scope
        || spec.operation != Operation::Infrastructure
        || spec.class != JobClass::InfrastructureOrdinary
        || spec.reservation_bytes != 65_536
        || spec.replacement.is_some()
        || spec.deadline != Some(loan.until)
        || context.deadline().is_none_or(|until| until > loan.until)
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(())
}
