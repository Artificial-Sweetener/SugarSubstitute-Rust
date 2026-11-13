// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Revalidate current authenticated transport independently of historical peer observations.
use super::*;
use sugarsubstitute_contracts::execution::{AdmissionError, JobPhase};

enum Rejection {
    Admission(AdmissionError),
    Pipe(LifecyclePipeError),
}
impl Rejection {
    fn retain(self) -> NativeObservationError {
        match self {
            Self::Admission(cause) => NativeObservationError::new("pipe_current_admission", cause),
            Self::Pipe(cause) => NativeObservationError::new("pipe_current_admission", cause),
        }
    }
}
impl LifecyclePipeControl {
    /// Check this actual authenticated transport immediately before application effects.
    /// Historical peer verification does not keep a stopped session eligible. The actual
    /// Core clock is sampled outside every owner lock; the final Core->Shared fence checks
    /// retained job identity, running phase, native slots, cancellation and all current
    /// scope ancestors. An authenticated passive session keeps its cleared handshake timer.
    /// This observation grants no handles or authority to defer later effect checks.
    /// # Errors
    /// Retains the actual provider/admission cause or typed closed/not-ready rejection.
    /// Error allocation happens after all Core/Shared guards are released.
    pub fn validate_current_admission(&self) -> Result<(), NativeObservationError> {
        validate(&self.inner)
    }
}

/// Apply one current-transport policy to every purpose-specific public projection.
/// Clock sampling and error construction remain outside the Core/Shared fences.
pub(crate) fn validate(control: &Control) -> Result<(), NativeObservationError> {
    validate_source(&control.shared, &control.owner)
}

/// Borrow original session parts without constructing a cancelling control owner.
pub(crate) fn validate_source(
    shared: &SharedRef,
    owner: &Owner,
) -> Result<(), NativeObservationError> {
    with_source(shared, owner, |_| Ok(()))
}

/// Apply a bounded metadata-only mutation under the final current-session fence.
/// The supplied crate-private operation must invoke no provider/native callback.
pub(crate) fn validated<T>(
    control: &Control,
    change: impl FnOnce(&mut State) -> Result<T, LifecyclePipeError>,
) -> Result<T, NativeObservationError> {
    with_source(&control.shared, &control.owner, change)
}

/// Retain the same final fence for metadata owners borrowing an original session.
fn with_source<T>(
    shared: &SharedRef,
    owner: &Owner,
    change: impl FnOnce(&mut State) -> Result<T, LifecyclePipeError>,
) -> Result<T, NativeObservationError> {
    let core = shared
        .core
        .upgrade()
        .ok_or_else(|| Rejection::Pipe(LifecyclePipeError::Closed).retain())?;
    let now = core
        .clock
        .metadata()
        .map_err(|cause| Rejection::Admission(AdmissionError::Unexpected(cause)).retain())?;
    let checked = (|| {
        let state = lock(&core.state);
        source_facts_locked(&core, &state, shared, owner, now)?;
        let mut pipe = lock(&shared.state);
        if pipe.stopped {
            return Err(Rejection::Pipe(LifecyclePipeError::Closed));
        }
        if !pipe.connected || !pipe.authenticated {
            return Err(Rejection::Pipe(LifecyclePipeError::NotReady));
        }
        change(&mut pipe).map_err(Rejection::Pipe)
    })();
    checked.map_err(Rejection::retain)
}

/// Revalidate original session facts under an existing final Core admission guard.
/// This borrows the same source allocation and performs no provider or native call.
pub(crate) fn source_locked(
    core: &Core,
    state: &crate::admission::State,
    shared: &SharedRef,
    owner: &Owner,
    now: Moment,
) -> Result<(), AdmissionError> {
    use sugarsubstitute_contracts::execution::NativeResourceError;
    source_facts_locked(core, state, shared, owner, now).map_err(|cause| match cause {
        Rejection::Admission(cause) => cause,
        Rejection::Pipe(_) => {
            AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired)
        }
    })
}

/// Share the exact provider-free current-source policy with admission and observation.
fn source_facts_locked(
    core: &Core,
    state: &crate::admission::State,
    shared: &SharedRef,
    owner: &Owner,
    now: Moment,
) -> Result<(), Rejection> {
    let closed = || Rejection::Pipe(LifecyclePipeError::Closed);
    let record = state.jobs.get(&shared.id).ok_or_else(closed)?;
    if !owner.matches_record(record) || record.phase != JobPhase::Running || record.retiring {
        return Err(closed());
    }
    let cutoff = crate::admission_liveness::scope_deadline(core, state, record)
        .map_err(Rejection::Admission)?;
    if let Some(deadline) = cutoff.filter(|deadline| *deadline <= now) {
        return Err(Rejection::Admission(AdmissionError::DeadlineExpired { deadline, now }));
    }
    let slots = record.native_deadlines.ok_or_else(closed)?;
    if slots.stopped()
        || crate::symbol_admission::original_stop(state, shared.id, now).is_some()
        || crate::coordinator_entry::recovery_grant::original_stop(
            core,
            state,
            shared.recovery_grant.as_ref(),
            now,
        )
        .is_some()
    {
        return Err(closed());
    }
    if let Some(deadline) = slots.expired_deadline(now) {
        return Err(Rejection::Admission(AdmissionError::DeadlineExpired { deadline, now }));
    }
    let pipe = lock(&shared.state);
    if pipe.stopped {
        return Err(closed());
    }
    if !pipe.connected || !pipe.authenticated {
        return Err(Rejection::Pipe(LifecyclePipeError::NotReady));
    }
    Ok(())
}

/// Validate an actual prepared Root server before child enrollment/authentication exists.
/// This separate private stage never relaxes the authenticated current-session fence.
#[cfg(windows)]
pub(crate) fn prepared_bootstrap_locked(
    core: &Core,
    state: &crate::admission::State,
    control: &Control,
    pin: &LifecyclePipeBootstrap,
    now: Moment,
) -> Result<(), AdmissionError> {
    if !control.shared.same_allocation(&pin.shared) {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    prepared_source_locked(core, state, control, now)
}

/// Recheck one genuinely owned prepared server while its actual pin is retained by the carrier.
/// The caller holds Core's final fence and separately owns/checks the original pair and enrollment.
pub(crate) fn prepared_source_locked(
    core: &Core,
    state: &crate::admission::State,
    control: &Control,
    now: Moment,
) -> Result<(), AdmissionError> {
    if control.shared.child
        || control.shared.core.upgrade().is_none_or(|actual| !std::ptr::eq(actual.as_ref(), core))
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let record =
        state.jobs.get(&control.shared.id).ok_or(AdmissionError::InvalidCaptureAdmission)?;
    if !control.owner.matches_record(record)
        || record.phase != JobPhase::Running
        || record.retiring
        || record.memory.is_none()
        || state.stopping
        || core.clock.failed()
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let cutoff = crate::admission_liveness::scope_deadline(core, state, record)?;
    if cutoff.is_some_and(|until| until <= now)
        || control.shared.original_deadline <= now
        || record
            .native_deadlines
            .is_none_or(|slots| slots.stopped() || slots.expired_deadline(now).is_some())
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let pipe = lock(&control.shared.state);
    if pipe.stopped
        || !pipe.prepared
        || !pipe.pin
        || pipe.inherited == 0
        || pipe.connected
        || pipe.authenticated
        || pipe.enrolled
        || pipe.peer.is_some()
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(())
}

/// Revalidate both original pre-ACK servers without requiring their not-yet-created peers.
/// Clock sampling precedes the single Core fence; current authenticated admission stays distinct.
pub(crate) fn prepared_recovery_pair(
    recovery: &Control,
    grant: &crate::RegisteredRecoveryGrant,
    desktop: &Control,
) -> Result<(), AdmissionError> {
    let core = recovery.shared.core.upgrade().ok_or(AdmissionError::InvalidCaptureAdmission)?;
    let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
    let state = lock(&core.state);
    grant.state()?.validate_locked(&core, &state, now)?;
    let original =
        recovery.shared.recovery_grant.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?;
    if !original.same_grant(grant)
        || recovery.shared.same_allocation(&desktop.shared)
        || desktop.shared.captured
        || recovery.shared.original_deadline != grant.original_budget().until()
        || desktop.shared.original_deadline != recovery.shared.original_deadline
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    prepared_source_locked(&core, &state, recovery, now)?;
    prepared_source_locked(&core, &state, desktop, now)?;
    let root = lock(&recovery.shared.state);
    let ui = lock(&desktop.shared.state);
    if root.binding != Some(PeerBinding::Recovery(grant.key()))
        || ui.binding != Some(PeerBinding::Desktop(grant.key().run()))
        || root.inherited == ui.inherited
        || root.deadline.is_none()
        || ui.deadline.is_none()
        || root.deadline.map(|deadline| deadline.frequency())
            != ui.deadline.map(|deadline| deadline.frequency())
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(())
}
