// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample supplied time before the final exact child stage acceptance fence.
use super::{
    NativeCaptureDriver,
    control::{AcceptedCaptureStage, DesktopCapturePhase},
    metadata::DesktopCaptureCustodyRef,
    native_abi::{NativeCaptureReply, NativeCaptureStep},
};
use crate::{admission::Core, native::NativeObservationError, scope::lock};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{CancelControl, JobContext, JobPhase};

/// Callback decision in the fixed C ABI; acceptance is distinct from issued I/O.
#[derive(Clone, Copy)]
pub(super) enum Decision {
    Accepted = 1,
    Stopped = 2,
    Expired = 3,
    Failed = 4,
}

/// Claim one bounded native step; finite Windows work occurs after this returns.
/// Only stage3 may accept nonblocking I/O, and stage6 updates a bounded native fact.
/// Native outputs are borrowed from a distinct preinitialized nested slot.
pub(super) fn execute(
    core: &Core,
    custody: &DesktopCaptureCustodyRef,
    context: &JobContext,
    driver: &dyn NativeCaptureDriver,
    request: AcceptedCaptureStage,
    output: &mut NativeCaptureReply,
    observed: sugarsubstitute_contracts::execution::Moment,
) -> Result<Decision, NativeObservationError> {
    let AcceptedCaptureStage { attach, stage, nonce } = request;
    if attach == 0 || !(1..=6).contains(&stage) || nonce == 0 {
        return Err(invariant("desktop_capture_stage_shape"));
    }
    let state = lock(&core.state);
    let record = state
        .jobs
        .get(&custody.cleanup.job)
        .ok_or_else(|| invariant("desktop_capture_issue_record"))?;
    if context.owner_id() != core.id
        || custody.cleanup.execution != core.id
        || record.scope_id != context.scope_id()
        || record.spec != context.spec()
        || record.deadline != context.deadline()
        || record.memory.as_ref().is_none_or(|memory| !memory.matches_lease(context.memory()))
        || record.desktop_capture_custody.as_ref().is_none_or(|actual| !actual.same(custody))
        || record
            .native_custody
            .as_ref()
            .is_none_or(|actual| !Arc::ptr_eq(actual, &custody.cleanup))
    {
        return Err(invariant("desktop_capture_issue_association"));
    }
    match crate::capture_bundle::validate::effect(core, &state, context, observed, true) {
        Ok(()) => {}
        Err(sugarsubstitute_contracts::execution::AdmissionError::DeadlineExpired { .. }) => {
            return Ok(Decision::Expired);
        }
        Err(
            sugarsubstitute_contracts::execution::AdmissionError::ScopeClosed { .. }
            | sugarsubstitute_contracts::execution::AdmissionError::ShuttingDown
            | sugarsubstitute_contracts::execution::AdmissionError::CaptureRetiring,
        ) => {
            return Ok(Decision::Stopped);
        }
        Err(cause) => return Err(NativeObservationError::new("desktop_capture_current", cause)),
    }
    let cleanup = lock(&custody.cleanup.state);
    let mut child = lock(&custody.state);
    if state.stopping
        || core.clock.failed()
        || record.retiring
        || record.phase != JobPhase::Running
        || !record.capture_bundle_issued
        || cleanup.stopped
        || state
            .scopes
            .get(&record.scope_id)
            .is_none_or(|scope| scope.closed || scope.abandoned || scope.cancel.is_requested())
        || record.job.cancellation().is_requested()
        || !matches!(child.phase, DesktopCapturePhase::Attaching)
        || record.native_deadlines.is_none_or(|deadlines| deadlines.stopped())
    {
        return Ok(Decision::Stopped);
    }
    if observed >= custody.cleanup.attach_until
        || state
            .scopes
            .get(&record.scope_id)
            .and_then(|scope| scope.deadline)
            .is_some_and(|until| observed >= until)
        || record.deadline.is_some_and(|until| observed >= until)
        || record.native_deadlines.is_some_and(|deadlines| deadlines.expired(observed))
    {
        return Ok(Decision::Expired);
    }
    if child.accepted_stage.is_some() || nonce <= child.last_nonce {
        return Err(invariant("desktop_capture_issue_replay"));
    }
    child.last_nonce = nonce;
    child.accepted_stage = Some(AcceptedCaptureStage { attach, stage, nonce });
    let step = NativeCaptureStep { version: 2, size: 32, attach, stage, reserved: 0, nonce };
    // This exact trusted C++ Step is a bounded claim/adoption. Finite stage work
    // follows in outer Poll only after every Rust fence has been released.
    driver.issue_step(&step, output)?;
    drop(child);
    drop(cleanup);
    drop(state);
    Ok(Decision::Accepted)
}
/// Preserve a shape or owner mismatch without translating it into a native cause.
fn invariant(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(stage, std::io::Error::other("Actual child capture issue rejected"))
}
