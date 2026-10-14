// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Issue sealed original specification and restart capsule only through actual Core records.
use super::*;
use crate::{ExecutionClient, capture_bundle::validate};
use sugarsubstitute_contracts::execution::{
    AdmissionError, CancelControl, JobClass, JobContext, Operation,
};

pub(in crate::coordinator_entry) fn scope_current(
    state: &State,
    mut id: ScopeId,
) -> Result<Option<Moment>, EntryTransportError> {
    let mut until = None;
    for _ in 0..196 {
        let scope = state.scopes.get(&id).ok_or(EntryTransportError::Closed)?;
        if scope.closed || scope.abandoned || scope.cancel.is_requested() {
            return Err(EntryTransportError::Closed);
        }
        until = crate::scope::earliest(until, scope.deadline);
        match scope.parent {
            Some(parent) => id = parent,
            None => return Ok(until),
        }
    }
    Err(EntryTransportError::Closed)
}
fn projection(
    core: &Core,
    state: &State,
    context: &JobContext,
    until: Moment,
    now: Moment,
) -> Result<(), AdmissionError> {
    validate::live(core, state, context, now)?;
    let record = validate::association(core, state, context)?;
    if record.spec.operation != Operation::NativeLifecycle
        || record.spec.class != JobClass::InfrastructureLifecycle
        || record.spec.reservation_bytes != PROJECTION_BYTES
        || record.spec.deadline != Some(until)
        || record.spec.replacement.is_some()
        || context.native_handles().is_some()
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(())
}
pub(super) fn transport(error: EntryTransportError) -> AdmissionError {
    AdmissionError::Unexpected(crate::runtime::error(error))
}
impl ExecutionClient {
    /// Register exact original authenticated startup bytes before their first native transfer.
    /// Claims the fixed 68 KiB envelope from the same actual 4 MiB projection, allocates
    /// outside Core and releases the original Entry loan independently of this source.
    /// # Errors
    /// Rejects counterfeit/foreign context, prior issuance, wrong fixed role/deadline,
    /// actual Entry ownership, current cancellation/clock failure or original exhaustion.
    pub fn register_startup_source(
        &self,
        snapshot: &PreparedStartupSnapshot,
        context: &JobContext,
        run: RunKey,
    ) -> Result<RegisteredStartupSource, AdmissionError> {
        if size_of::<StateOwned>()
            + size_of::<RegisteredSnapshot>()
            + size_of::<RegisteredRestartAdmission<'_>>()
            + 2 * size_of::<usize>()
            > 4096
            || snapshot.body().map_err(transport)?.len() > 65_536
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let entry = snapshot.admission_binding().map_err(transport)?;
        if !matches!(entry, StartupAdmissionBinding::Entry(_)) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let until =
            snapshot.launch.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?.until();
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let claim;
        {
            let mut state = lock(&self.core.state);
            projection(&self.core, &state, context, until, now)?;
            entry.validate_current_locked(&self.core, &state, run, now).map_err(transport)?;
            let record = state
                .jobs
                .get_mut(&context.job_id())
                .ok_or(AdmissionError::InvalidCaptureAdmission)?;
            if record.registered_startup_issued {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            claim =
                context.memory().claim(REGISTERED_STARTUP_BYTES).map_err(AdmissionError::Memory)?;
            record.registered_startup_issued = true;
        }
        let original_body = snapshot.body().map_err(transport)?;
        let mut body = Vec::new();
        body.try_reserve_exact(original_body.len())
            .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        body.extend_from_slice(original_body);
        let body = body.into_boxed_slice();
        Ok(RegisteredStartupSource(Some(Arc::new(StateOwned {
            core: self.core.clone(),
            entry,
            original_scope: context.scope_id(),
            body,
            progress: Mutex::new(Progress { current: run, issued: None }),
            _memory: claim,
        }))))
    }
    /// Issue one captured restart capsule for the registered source's exact next generation.
    /// Its first coordinator-issued 30-second ancestor remains unchanged through retries;
    /// historical authentication is retained without constructing a new Entry request.
    /// # Errors
    /// Rejects foreign source/owner/scope, stale or repeated generation, actual projection
    /// cancellation, mismatched original budget, closed Root Entry owner or clock failure.
    pub fn restart_startup_snapshot(
        &self,
        source: &RegisteredStartupSource,
        previous: RunKey,
        run: RunKey,
        original: EntrySetupBudget,
        context: &JobContext,
    ) -> Result<PreparedStartupSnapshot, AdmissionError> {
        let data = source.state().map_err(transport)?;
        if !Arc::ptr_eq(&data.core, &self.core)
            || run.owner != previous.owner
            || run.instance != previous.instance
            || previous.generation.next().ok() != Some(run.generation)
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let (cancel, memory, issuer) = {
            let state = lock(&self.core.state);
            projection(&self.core, &state, context, original.until(), now)?;
            if now < original.started() {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            data.entry
                .validate_registered_owner_locked(&self.core, &state, run, now)
                .map_err(transport)?;
            if scope_current(&state, data.original_scope)
                .map_err(transport)?
                .is_some_and(|until| now >= until)
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let mut progress = lock(&data.progress);
            if progress.current != previous {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            progress.current = run;
            progress.issued = Some((context.job_id(), context.scope_id(), original));
            let record = validate::association(&self.core, &state, context)?;
            (record.job.cancellation(), context.memory().clone(), record.job.clone())
        };
        PreparedStartupSnapshot::registered(RegisteredSnapshot {
            source: source.clone(),
            previous,
            run,
            scope: context.scope_id(),
            job: context.job_id(),
            original,
            cancel,
            _issuer: issuer,
            _memory: memory,
        })
        .map_err(transport)
    }
}
