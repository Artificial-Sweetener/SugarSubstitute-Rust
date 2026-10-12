// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Admit fixed symbol work and transfer original output accounting through the sole execution owner.

use crate::{
    ExecutionClient,
    admission::{Core, RequestKind, State},
    scope::lock,
    symbol_cleanup::SymbolCleanupPermit,
    task::{Job, JobHandle, QueuedJob, Work},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, CancelControl, JobClass, JobContext, JobPhase, JobSpec, Moment,
    NativeHandleClaim, NativeResourceError, Operation, OwnedJob, Scope, SymbolNativePartition,
};

pub(crate) const MAIN_BYTES: u64 = 8_159_232;
pub(crate) const CLEANUP_BYTES: u64 = 32_768;

/// Return an unexecuted factory when fixed admission rejects its original request.
pub struct SymbolSubmissionRejection<M> {
    /// Actual admission decision or preserved owner failure.
    pub error: AdmissionError,
    /// The exact submitted factory, with all captured ownership intact.
    pub factory: M,
}

/// Return original partition custody when its actual Core provenance is rejected.
pub struct SymbolResourceRejection {
    /// Actual admission decision or preserved owner failure.
    pub error: AdmissionError,
    /// The exact noncloneable partition supplied by the caller.
    pub handles: NativeHandleClaim,
}

/// Retain the unique transport share of an actual original symbol admission.
/// Construction validates actual Core ledger identity; accounting grants no OS privilege.
pub struct SymbolTransportReservation {
    pub(crate) handles: Arc<NativeHandleClaim>,
    pub(crate) original: Box<JobContext>,
    pub(crate) core: std::sync::Weak<Core>,
}

/// Select ordinary diagnostics or the unique actual original symbol transport share.
pub enum DiagnosticPipeResources {
    /// Ordinary diagnostic transport with its standard byte admission.
    Standard,
    /// Actual original symbol accounting, retained through native retirement.
    Symbol(SymbolTransportReservation),
}

pub(crate) fn original_stop(
    state: &State,
    native_job: sugarsubstitute_contracts::execution::JobId,
    now: Moment,
) -> Option<crate::task::Stop> {
    let original = state.jobs.get(&native_job).and_then(|record| record.symbol_original)?;
    let Some(main) = state.jobs.get(&original) else {
        return Some(crate::task::Stop::Cancelled);
    };
    if main.deadline.is_some_and(|deadline| now >= deadline) {
        return Some(crate::task::Stop::TimedOut);
    }
    if main.retiring
        || main.phase != JobPhase::Running
        || main.job.cancellation().is_requested()
        || state
            .scopes
            .get(&main.scope_id)
            .is_none_or(|scope| scope.closed || scope.cancel.is_requested())
        || !state.symbol_resources.contains_key(&original)
    {
        return Some(crate::task::Stop::Cancelled);
    }
    None
}

fn valid_main(spec: JobSpec) -> bool {
    spec.operation == Operation::Infrastructure
        && spec.class == JobClass::InfrastructureOrdinary
        && spec.reservation_bytes == MAIN_BYTES
        && spec.deadline.is_some()
        && spec.replacement.is_none()
}

fn descendant(
    state: &State,
    mut child: sugarsubstitute_contracts::execution::ScopeId,
    ancestor: sugarsubstitute_contracts::execution::ScopeId,
) -> bool {
    loop {
        if child == ancestor {
            return true;
        }
        match state.scopes.get(&child).and_then(|scope| scope.parent) {
            Some(parent) => child = parent,
            None => return false,
        }
    }
}

pub(crate) fn validate(
    core: &Core,
    state: &State,
    scope: &Scope,
    spec: JobSpec,
    kind: &RequestKind,
    now: Moment,
) -> Result<(), AdmissionError> {
    match kind {
        RequestKind::SymbolCleanup { main_scope, main_spec } => {
            let main = state
                .scopes
                .get(&main_scope.id)
                .filter(|stored| stored.spec == *main_scope)
                .ok_or(AdmissionError::ScopeClosed { scope_id: main_scope.id })?;
            if main.closed || main.cancel.is_requested() {
                return Err(AdmissionError::ScopeClosed { scope_id: main_scope.id });
            }
            if !valid_main(*main_spec)
                || main.native
                || main.deadline.is_some()
                || scope.spec().deadline.is_some()
                || state.scopes.get(&scope.id()).is_none_or(|stored| stored.deadline.is_some())
                || descendant(state, scope.id(), main_scope.id)
                || descendant(state, main_scope.id, scope.id())
                || spec.operation != Operation::Infrastructure
                || spec.class != JobClass::InfrastructureLifecycle
                || spec.reservation_bytes != CLEANUP_BYTES
                || spec.deadline.is_some()
                || spec.replacement.is_some()
                || spec.id >= main_spec.id
            {
                return Err(AdmissionError::InvalidSymbolAdmission);
            }
            if let Some(deadline) = main_spec.deadline.filter(|deadline| *deadline <= now) {
                return Err(AdmissionError::DeadlineExpired { deadline, now });
            }
            if state.jobs.contains_key(&main_spec.id)
                || state
                    .jobs
                    .values()
                    .filter_map(|record| record.symbol_cleanup.as_ref())
                    .any(|binding| binding.main_spec.id == main_spec.id)
            {
                return Err(AdmissionError::DuplicateJobId { job_id: main_spec.id });
            }
        }
        RequestKind::SymbolHelper { cleanup } => {
            let held = state.jobs.get(cleanup).ok_or(AdmissionError::InvalidSymbolAdmission)?;
            let binding =
                held.symbol_cleanup.as_ref().ok_or(AdmissionError::InvalidSymbolAdmission)?;
            let cleanup_scope = state
                .scopes
                .get(&held.scope_id)
                .ok_or(AdmissionError::ScopeClosed { scope_id: held.scope_id })?;
            if cleanup_scope.closed
                || cleanup_scope.cancel.is_requested()
                || held.job.cancellation().is_requested()
                || !valid_main(spec)
                || state.scopes.get(&scope.id()).is_none_or(|stored| stored.deadline.is_some())
                || binding.paired
                || binding.main_spec != spec
                || binding.main_scope != scope.spec()
                || held.phase != JobPhase::Queued
                || held.retiring
                || !state.queue.iter().any(|queued| {
                    queued.context.job_id() == *cleanup
                        && matches!(queued.work, Work::BlockingHeld(_))
                })
            {
                return Err(AdmissionError::InvalidSymbolAdmission);
            }
        }
        RequestKind::SymbolNative { original, handles } => {
            validate_context(core, state, original, handles, now)?;
            if scope.spec().deadline.is_some()
                || spec.deadline.is_some()
                || spec.reservation_bytes != 196_608
                || scope.id() == original.scope_id()
                || !descendant(state, scope.id(), original.scope_id())
                || handles.in_custody() != 0
            {
                return Err(AdmissionError::InvalidSymbolAdmission);
            }
        }
        _ => {}
    }
    Ok(())
}

impl ExecutionClient {
    /// Admit fixed finite symbol work only after its actual independent cleanup job is held.
    ///
    /// # Errors
    /// Rejects invalid fixed specifications, foreign or unpaired cleanup, and actual
    /// capacity/cancellation/deadline failures, returning the exact unexecuted factory.
    pub fn submit_symbol_helper<T, E, M, CT, CE>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        cleanup: &SymbolCleanupPermit<CT, CE>,
        factory: M,
    ) -> Result<OwnedJob<T, E>, SymbolSubmissionRejection<M>>
    where
        T: Send + 'static,
        E: Send + 'static,
        CT: Send + 'static,
        CE: Send + 'static,
        M: FnOnce(JobContext) -> Result<T, E> + Send + 'static,
    {
        if !cleanup.belongs_to(&self.core) {
            return Err(SymbolSubmissionRejection {
                error: AdmissionError::InvalidSymbolAdmission,
                factory,
            });
        }
        self.core
            .admit_owned(
                scope,
                spec,
                RequestKind::SymbolHelper { cleanup: cleanup.original_job_id() },
                factory,
                |factory, context, cancel, admitted| {
                    let job =
                        Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(&self.core));
                    let task_job = job.clone();
                    let queued = QueuedJob {
                        context,
                        job: job.clone(),
                        work: Work::Blocking(Box::new(move |context, started| {
                            Box::pin(crate::blocking::run(task_job, context, started, factory))
                        })),
                    };
                    (queued, OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))))
                },
            )
            .map_err(|(error, factory)| SymbolSubmissionRejection { error, factory })
    }

    /// Consume the unique transport partition of the actual currently running symbol job.
    ///
    /// # Errors
    /// Rejects wrong ledger, partition, context, terminal admission or current owner
    /// cancellation/deadline failure. Rejection returns the exact original partition.
    pub fn symbol_transport_reservation(
        &self,
        context: &JobContext,
        handles: NativeHandleClaim,
    ) -> Result<SymbolTransportReservation, SymbolResourceRejection> {
        let decision =
            self.core.clock.metadata().map_err(AdmissionError::Unexpected).and_then(|now| {
                let state = lock(&self.core.state);
                validate_context(&self.core, &state, context, &handles, now)
            });
        match decision {
            Ok(()) => Ok(SymbolTransportReservation {
                handles: Arc::new(handles),
                original: Box::new(context.clone()),
                core: Arc::downgrade(&self.core),
            }),
            Err(error) => Err(SymbolResourceRejection { error, handles }),
        }
    }
}

pub(crate) fn validate_context(
    core: &Core,
    state: &State,
    context: &JobContext,
    handles: &NativeHandleClaim,
    now: Moment,
) -> Result<(), AdmissionError> {
    let invalid = || AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired);
    if context.owner_id() != core.id {
        return Err(AdmissionError::NativeResources(NativeResourceError::ForeignOwner {
            expected: core.id,
            actual: context.owner_id(),
        }));
    }
    if state.stopping || core.clock.failed() {
        return Err(AdmissionError::ShuttingDown);
    }
    let record = state.jobs.get(&context.job_id()).ok_or_else(invalid)?;
    let scope = state.scopes.get(&context.scope_id()).ok_or_else(invalid)?;
    let resource = state.symbol_resources.get(&context.job_id()).ok_or_else(invalid)?;
    let lease = context.native_handles().ok_or_else(invalid)?;
    if record.scope_id != context.scope_id()
        || record.spec != context.spec()
        || !valid_main(record.spec)
        || record.deadline != context.deadline()
        || record.phase != JobPhase::Running
        || record.retiring
        || resource.scope_id != context.scope_id()
        || !resource.observation.matches_lease(lease)
        || !handles.matches_lease(lease)
        || lease.owner_id() != core.id
        || lease.original_job_id() != context.job_id()
        || lease.original_scope_id() != context.scope_id()
    {
        return Err(invalid());
    }
    if handles.partition() != SymbolNativePartition::Transport {
        return Err(AdmissionError::NativeResources(NativeResourceError::WrongPartition {
            expected: SymbolNativePartition::Transport,
            actual: handles.partition(),
        }));
    }
    if scope.closed
        || scope.cancel.is_requested()
        || record.job.cancellation().is_requested()
        || context.cancellation_requested()
    {
        return Err(AdmissionError::ScopeClosed { scope_id: context.scope_id() });
    }
    if let Some(deadline) = record.deadline.filter(|deadline| *deadline <= now) {
        return Err(AdmissionError::DeadlineExpired { deadline, now });
    }
    Ok(())
}

#[cfg(test)]
#[path = "symbol_admission_tests.rs"]
mod tests;
