// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence the actual parent pin and original Entry loan before native session construction.
use crate::{
    admission::{Core, State},
    capture_attach::Shared,
    coordinator_entry::StartupAdmissionBinding,
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    crash::CaptureTarget,
    execution::{AdmissionError, CancelControl, JobPhase, Moment, TerminalKind},
    lifecycle::RunKey,
};

/// Inert aliases are projected from the still-owned nonclone pin/snapshot input.
/// Their original allocations/claims remain outside Core in that complete input.
/// No caller can construct this binding from PID/endpoint/build matching facts.
#[derive(Clone)]
pub(crate) struct AdmissionBinding {
    target: CaptureTarget,
    original_handshake: Moment,
    capture: Arc<Shared>,
    startup: Option<StartupAdmissionBinding>,
}
impl AdmissionBinding {
    /// Return the exact original reservation for this sealed target's native purpose.
    pub(crate) const fn reservation_bytes(&self) -> u64 {
        match self.target {
            CaptureTarget::Desktop(_) => crate::lifecycle_pipe::MIN_CAPTURED_LIFECYCLE_PIPE_BYTES,
            CaptureTarget::BackendCustody(_) => crate::lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES,
        }
    }
    /// Borrow already genuine parent owners without moving or allocating payloads.
    pub(crate) fn project(
        run: RunKey,
        original_handshake: Moment,
        pin: &crate::capture_attach::CaptureAttachLaunchPin,
        startup: StartupAdmissionBinding,
    ) -> Self {
        Self {
            target: CaptureTarget::Desktop(run),
            original_handshake,
            capture: pin.shared.clone(),
            startup: Some(startup),
        }
    }
    /// Borrow the same actual pin before purpose-specific backend server admission.
    pub(crate) fn backend(
        key: sugarsubstitute_contracts::backend_custody::BackendCustodyKey,
        original_handshake: Moment,
        pin: &crate::capture_attach::CaptureAttachLaunchPin,
    ) -> Self {
        Self {
            target: CaptureTarget::BackendCustody(key),
            original_handshake,
            capture: pin.shared.clone(),
            startup: None,
        }
    }
    /// Return only the original phase-specific handshake under the actual Core guard.
    /// Persistent raw JobSpec/context deadlines remain unchanged; real authentication
    /// clears this existing handshake slot, without renewing any interval.
    pub(crate) fn validate_current_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        now: Moment,
    ) -> Result<Moment, AdmissionError> {
        let invalid = AdmissionError::InvalidCaptureAdmission;
        let custody = &self.capture.custody;
        if custody.execution != core.id
            || custody.full_target != self.target
            || self.capture.expected.target() != self.target
            || custody.attach_until != self.capture.expected.attach_budget().until()
            || custody.core.upgrade().is_none_or(|actual| !Arc::ptr_eq(core, &actual))
        {
            return Err(invalid);
        }
        let record = state.jobs.get(&custody.job).ok_or(AdmissionError::InvalidCaptureAdmission)?;
        if record.native_custody.as_ref().is_none_or(|actual| !Arc::ptr_eq(actual, custody))
            || !record.terminal_published
            || record.phase != JobPhase::Terminal(TerminalKind::Success)
            || record.retiring
            || record.job.cancellation().is_requested()
            || record.memory.is_none()
            || state.stopping
            || core.clock.failed()
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let mut until = self.original_handshake.min(custody.attach_until);
        let mut parent = Some(record.scope_id);
        let mut ancestors = 0;
        while let Some(id) = parent {
            if ancestors >= 64 + 132 {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            ancestors += 1;
            let scope = state.scopes.get(&id).ok_or(AdmissionError::InvalidCaptureAdmission)?;
            if scope.closed || scope.abandoned || scope.cancel.is_requested() {
                return Err(AdmissionError::ScopeClosed { scope_id: id });
            }
            if let Some(deadline) = scope.deadline {
                until = until.min(deadline);
            }
            parent = scope.parent;
        }
        let entry_until = match (self.target, &self.startup) {
            (CaptureTarget::Desktop(run), Some(startup)) => startup
                .validate_current_locked(core, state, run, now)
                .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?,
            (CaptureTarget::BackendCustody(_), None) => until,
            _ => return Err(AdmissionError::InvalidCaptureAdmission),
        };
        let cleanup = lock(&custody.state);
        let pin = lock(&self.capture.state);
        if cleanup.stopped
            || !cleanup.held()
            || !pin.pinned
            || pin.handles.is_none()
            || pin.endpoint.is_none_or(|endpoint| endpoint.expectation() != self.capture.expected)
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let until = until.min(entry_until);
        if until <= now {
            return Err(AdmissionError::DeadlineExpired { deadline: until, now });
        }
        Ok(until)
    }
}
