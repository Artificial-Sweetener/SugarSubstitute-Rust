// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry genuine provider-free admission timing into the same owner's constructor.
use crate::admission::{Core, RequestKind, State};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, Moment, ScopeId};
#[cfg(any(windows, test))]
use sugarsubstitute_contracts::execution::{JobClass, Operation};

/// Matching timing latched before IDs, charges, construction and native ownership transfer.
/// Only the phase-specific native slot uses `native_handshake`; original persistent
/// JobSpec and JobContext deadlines are not replaced or renewed.
pub(crate) struct AdmittedTiming {
    pub(crate) admitted: Moment,
    #[cfg(any(windows, test))]
    pub(crate) native_handshake: Option<Moment>,
}

/// Validate the actual captured-parent source under the caller's Core state guard.
/// This invokes only the constructor-private binding's passive owner checks.
/// No clock, native effect, allocation or public-context reconstruction occurs here.
pub(crate) fn validate(
    core: &Arc<Core>,
    state: &State,
    receiving_scope: ScopeId,
    spec: JobSpec,
    kind: &RequestKind<'_>,
    effective_deadline: Option<Moment>,
    now: Moment,
) -> Result<AdmittedTiming, AdmissionError> {
    #[cfg(any(windows, test))]
    if let RequestKind::RecoveryPipe(grant) = kind {
        let grant = grant.state()?;
        let until = grant.validate_locked(core, state, now)?;
        if spec.operation != Operation::NativeLifecycle
            || spec.class != JobClass::NativeObservation
            || spec.reservation_bytes != crate::lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES
            || effective_deadline.is_some()
            || spec.deadline.is_some()
            || spec.replacement.is_some()
            || grant.stages.lock().map_or(true, |stages| stages.pipe.is_some() || stages.closed)
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let cutoff = receiving_cutoff(state, receiving_scope, now)?;
        return Ok(AdmittedTiming {
            admitted: now,
            native_handshake: Some(cutoff.map_or(until, |c| c.min(until))),
        });
    }
    #[cfg(any(windows, test))]
    if let RequestKind::BackendCustody { source, original } = kind {
        crate::backend_custody::submission::validate_locked(core, state, source, original, now)?;
        if receiving_cutoff(state, receiving_scope, now)?.is_some() || effective_deadline.is_some()
        {
            return Err(AdmissionError::NativeResources(
                sugarsubstitute_contracts::execution::NativeResourceError::OriginalAdmissionRetired,
            ));
        }
    }
    #[cfg(any(windows, test))]
    if let RequestKind::ProcessCreation(source) = kind {
        source.admit_locked(core, state, spec, now)?;
        let cutoff = crate::scope::earliest(
            receiving_cutoff(state, receiving_scope, now)?,
            effective_deadline,
        );
        if spec.operation != Operation::NativeLifecycle
            || spec.class != JobClass::InfrastructureLifecycle
            || cutoff.is_none()
            || spec.replacement.is_some()
        {
            return Err(AdmissionError::NativeResources(
                sugarsubstitute_contracts::execution::NativeResourceError::OriginalAdmissionRetired,
            ));
        }
    }
    #[cfg(any(windows, test))]
    let native_handshake = if let RequestKind::CapturedLifecycle(binding) = kind {
        if spec.class != JobClass::NativeObservation
            || spec.operation != Operation::NativeLifecycle
            || spec.reservation_bytes != binding.reservation_bytes()
            || spec.deadline.is_some()
            || effective_deadline.is_some()
            || spec.replacement.is_some()
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let cutoff = receiving_cutoff(state, receiving_scope, now)?;
        let until = binding.validate_current_locked(core, state, now)?;
        Some(cutoff.map_or(until, |cutoff| until.min(cutoff)))
    } else {
        None
    };
    #[cfg(not(any(windows, test)))]
    {
        let _ = (core, state, receiving_scope, spec, kind, effective_deadline);
    };
    Ok(AdmittedTiming {
        admitted: now,
        #[cfg(any(windows, test))]
        native_handshake,
    })
}

/// Check the actual receiving ancestry before any captured job identity or charge.
/// A later narrowed ancestor caps only the original native handshake; persistent
/// raw JobSpec and JobContext deadlines stay unchanged. No provider runs here.
#[cfg(any(windows, test))]
fn receiving_cutoff(
    state: &State,
    scope_id: ScopeId,
    now: Moment,
) -> Result<Option<Moment>, AdmissionError> {
    use sugarsubstitute_contracts::execution::CancelControl;
    let mut current = Some(scope_id);
    let mut cutoff = None;
    let mut ancestors = 0;
    while let Some(id) = current {
        if ancestors >= 64 + 132 {
            return Err(AdmissionError::ScopeClosed { scope_id });
        }
        ancestors += 1;
        let scope = state.scopes.get(&id).ok_or(AdmissionError::ScopeClosed { scope_id })?;
        if scope.closed || scope.abandoned || scope.cancel.is_requested() {
            return Err(AdmissionError::ScopeClosed { scope_id: id });
        }
        cutoff = crate::scope::earliest(cutoff, scope.deadline);
        current = scope.parent;
    }
    if let Some(deadline) = cutoff.filter(|deadline| *deadline <= now) {
        return Err(AdmissionError::DeadlineExpired { deadline, now });
    }
    Ok(cutoff)
}

#[cfg(test)]
#[path = "admission_timing_tests.rs"]
mod tests;
