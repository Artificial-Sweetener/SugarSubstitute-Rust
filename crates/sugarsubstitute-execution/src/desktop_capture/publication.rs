// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Publish actual registration and current eligibility under one final terminal fence.

use super::{call::CallOutputs, control::DesktopCapturePhase, metadata::DesktopCaptureCustodyRef};
use crate::{
    admission::Core, capture_attach::types::CaptureAttachFailure, native::NativeObservationError,
    scope::lock, task::Job,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext, JobOutcome, Moment};

/// Keep the original prepared fields on veto; no native call or clock is made here.
/// `observed` is sampled after actual Poll return and before any owner lock.
pub(super) fn registered(
    core: &Core,
    job: &Job<super::types::DesktopCaptureAttached, CaptureAttachFailure>,
    custody: &DesktopCaptureCustodyRef,
    context: &JobContext,
    started: Moment,
    observed: Moment,
    outputs: &CallOutputs,
) -> Result<bool, NativeObservationError> {
    let facts = super::native_reply::decode(&outputs.outer).map_err(|cause| {
        NativeObservationError::new("desktop_capture_registration_reply", cause)
    })?;
    if !facts.registered
        || facts.retired
        || facts.pending
        || facts.primary.is_some()
        || outputs.attach == 0
        || outputs.outer.attach != outputs.attach
        || outputs.foreign_attach
    {
        return Err(NativeObservationError::new(
            "desktop_capture_registration_facts",
            std::io::Error::other("Actual native registration is incomplete"),
        ));
    }
    let result = super::types::DesktopCaptureAttached {
        run: custody.cleanup.full_run,
        execution: context.owner_id(),
        job: context.job_id(),
    };
    job.finish_current_at(context, started, observed, JobOutcome::Success(result), |state| {
        // This authoritative validator locks cleanup/stage internally. It must
        // finish before taking those same subordinate locks for publication.
        crate::capture_bundle::validate::effect(core, state, context, observed, true)?;
        let record = crate::capture_bundle::validate::association(core, state, context)?;
        if record.desktop_capture_custody.as_ref().is_none_or(|actual| !actual.same(custody))
            || record
                .native_custody
                .as_ref()
                .is_none_or(|actual| !Arc::ptr_eq(actual, &custody.cleanup))
            || custody.cleanup.execution != core.id
            || custody.cleanup.job != context.job_id()
            || observed >= custody.cleanup.attach_until
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let cleanup = lock(&custody.cleanup.state);
        let mut stage = lock(&custody.state);
        if cleanup.stopped
            || stage.registered_published
            || stage.phase != DesktopCapturePhase::Attaching
            || stage.accepted_stage.is_some()
            || stage.accepted_cleanup.is_some()
        {
            return Err(AdmissionError::CaptureRetiring);
        }
        stage.registered_published = true;
        stage.phase = DesktopCapturePhase::Registered;
        if let Some(deadlines) = state
            .jobs
            .get_mut(&context.job_id())
            .and_then(|record| record.native_deadlines.as_mut())
        {
            deadlines.clear(crate::native_deadlines::Role::Handshake, 0);
        }
        Ok(())
    })
    .map_err(|(cause, _original_outcome)| {
        NativeObservationError::new("desktop_capture_registration_current", cause)
    })
}
