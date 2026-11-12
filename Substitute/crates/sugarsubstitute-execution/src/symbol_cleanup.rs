// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Hold one actually admitted independent cleanup factory until its immutable first deadline activates.

use crate::{
    ExecutionClient,
    admission::{Core, RequestKind, State},
    scope::lock,
    symbol_admission::SymbolSubmissionRejection,
    task::{Job, JobHandle, QueuedJob, Work},
};
mod reservation;
#[cfg(test)]
mod reservation_tests;

use std::{
    sync::{Arc, Weak},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, AdmittedJobResources, CancelControl, ExecutionId, JobCompletion, JobContext,
    JobId, JobPhase, JobSpec, JobStatus, Moment, OwnedJob, Scope, ScopeId, ScopeSpec,
};

pub(crate) struct Binding {
    pub(crate) report_export_partner: Option<JobId>,
    pub(crate) main_scope: ScopeSpec,
    pub(crate) main_spec: JobSpec,
    pub(crate) paired: bool,
    pub(crate) original: Option<CleanupDeadline>,
}

/// Own the actual held cleanup job, its terminal observation and original admission.
/// Dropping requests cancellation; observe actual retirement through `cancel_and_observe`.
#[must_use = "Observe the actual held cleanup job through activation or cancellation"]
pub struct SymbolCleanupPermit<T: Send + 'static, E: Send + 'static> {
    job: Box<OwnedJob<T, E>>,
    core: Weak<Core>,
}

/// Carry the first immutable cleanup sample in the original execution clock epoch.
/// Private fields bind this token to its actual pre-held job; copying cannot renew time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupDeadline {
    owner_id: ExecutionId,
    scope_id: ScopeId,
    job_id: JobId,
    started: Moment,
    maximum: Duration,
    deadline: Moment,
}
impl CleanupDeadline {
    /// Match passive immutable token facts to its actual admitted cleanup record.
    pub(crate) fn matches_actual(&self, owner: ExecutionId, scope: ScopeId, job: JobId) -> bool {
        self.owner_id == owner && self.scope_id == scope && self.job_id == job
    }
    /// Return the first cleanup observation, in original monotonic nanoseconds.
    pub const fn started(&self) -> Moment {
        self.started
    }
    /// Return the first immutable deadline, in the same clock epoch.
    pub const fn deadline(&self) -> Moment {
        self.deadline
    }
    /// Return the original positive cleanup budget, at most fifteen seconds.
    pub const fn maximum(&self) -> Duration {
        self.maximum
    }
}

/// Preserve the exact held job and its observation when activation rejects.
pub struct SymbolCleanupBeginRejection<T: Send + 'static, E: Send + 'static> {
    /// Actual admission decision or preserved clock failure.
    pub error: AdmissionError,
    /// The identical pre-held permit, with factory and job ownership intact.
    pub permit: SymbolCleanupPermit<T, E>,
}

fn held<'a>(
    core: &Core,
    state: &'a mut State,
    id: JobId,
    scope_id: ScopeId,
) -> Result<&'a mut Binding, AdmissionError> {
    let paired = state.jobs.get(&id).and_then(|record| record.symbol_cleanup.as_ref());
    if let Some(binding) = paired
        && let Some(partner) = binding.report_export_partner
        && [binding.main_spec.id, partner].iter().any(|id| {
            state.jobs.get(id).is_some_and(|record| !matches!(record.phase, JobPhase::Terminal(_)))
        })
    {
        return Err(AdmissionError::InvalidSymbolAdmission);
    }
    if state.stopping || core.clock.failed() {
        return Err(AdmissionError::ShuttingDown);
    }
    let scope = state.scopes.get(&scope_id).ok_or(AdmissionError::ScopeClosed { scope_id })?;
    if scope.closed || scope.cancel.is_requested() {
        return Err(AdmissionError::ScopeClosed { scope_id });
    }
    let record = state.jobs.get_mut(&id).ok_or(AdmissionError::InvalidSymbolAdmission)?;
    if record.scope_id != scope_id
        || record.phase != JobPhase::Queued
        || record.retiring
        || record.job.cancellation().is_requested()
        || !state.queue.iter().any(|queued| {
            queued.context.job_id() == id && matches!(queued.work, Work::BlockingHeld(_))
        })
    {
        return Err(AdmissionError::InvalidSymbolAdmission);
    }
    let binding = record.symbol_cleanup.as_mut().ok_or(AdmissionError::InvalidSymbolAdmission)?;
    if !binding.paired {
        return Err(AdmissionError::InvalidSymbolAdmission);
    }
    Ok(binding)
}

impl ExecutionClient {
    /// Hold independent cleanup before admitting its fixed original symbol job.
    /// The factory consumes normal lifecycle queue, completion and byte credits,
    /// but cannot execute until activation. Cancellation destroys it on the actual blocking lane.
    ///
    /// # Errors
    /// Rejects invalid independent scope/specification, foreign main scope, expired
    /// original main deadline, or capacity failure, returning the unexecuted factory.
    pub fn reserve_symbol_cleanup<T, E, M>(
        &self,
        cleanup_scope: &Scope,
        cleanup_spec: JobSpec,
        main_scope: &Scope,
        main_spec: JobSpec,
        factory: M,
    ) -> Result<SymbolCleanupPermit<T, E>, SymbolSubmissionRejection<M>>
    where
        T: Send + 'static,
        E: Send + 'static,
        M: FnOnce(JobContext) -> Result<T, E> + Send + 'static,
    {
        self.reserve_symbol_cleanup_construct(
            cleanup_scope,
            cleanup_spec,
            main_scope,
            main_spec,
            factory,
            |factory, _context| Ok((factory, ())),
        )
        .map(|(_source, permit)| permit)
    }

    /// Freeze the first independent cleanup deadline after successful paired main admission.
    /// Samples the actual supplied clock outside Core locks and revalidates current
    /// held state afterward. Identical repeats retain the first token and never renew time.
    ///
    /// # Errors
    /// Rejects foreign or unpaired jobs, zero or greater-than-fifteen-second budgets,
    /// changed duration, cancellation/stopping, elapsed first deadline or clock failure.
    pub fn freeze_symbol_cleanup_deadline<T: Send + 'static, E: Send + 'static>(
        &self,
        permit: &SymbolCleanupPermit<T, E>,
        maximum: Duration,
    ) -> Result<CleanupDeadline, AdmissionError> {
        if !permit.belongs_to(&self.core) || maximum.is_zero() || maximum > Duration::from_secs(15)
        {
            return Err(AdmissionError::InvalidSymbolAdmission);
        }
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let mut state = lock(&self.core.state);
        let binding =
            held(&self.core, &mut state, permit.original_job_id(), permit.original_scope_id())?;
        if let Some(original) = binding.original {
            if original.maximum != maximum {
                return Err(AdmissionError::InvalidSymbolAdmission);
            }
            if now >= original.deadline {
                return Err(AdmissionError::DeadlineExpired { deadline: original.deadline, now });
            }
            return Ok(original);
        }
        let deadline = now
            .checked_add(maximum)
            .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        let original = CleanupDeadline {
            owner_id: self.core.id,
            scope_id: permit.original_scope_id(),
            job_id: permit.original_job_id(),
            started: now,
            maximum,
            deadline,
        };
        binding.original = Some(original);
        Ok(original)
    }
}

impl<T: Send + 'static, E: Send + 'static> SymbolCleanupPermit<T, E> {
    pub(crate) fn belongs_to(&self, core: &Arc<Core>) -> bool {
        Weak::ptr_eq(&self.core, &Arc::downgrade(core))
    }
    /// Return the independent scope of the actual pre-held cleanup job.
    pub fn original_scope_id(&self) -> ScopeId {
        self.job.scope_id()
    }
    /// Return the identity of that same actual job throughout its lifetime.
    pub fn original_job_id(&self) -> JobId {
        self.job.spec().id
    }
    /// Observe actual queued/running/terminal status without transferring ownership.
    pub fn status(&self) -> JobStatus {
        self.job.status()
    }
    /// Request actual held/running cleanup cancellation without claiming retirement.
    pub fn request_cancel(&self) {
        self.job.cancel();
    }
    /// Transfer actual terminal completion and acknowledge its delivery.
    pub fn try_take_completion(&mut self) -> Option<JobCompletion<T, E>> {
        self.job.try_take_completion()
    }
    /// Request cancellation and transfer the identical admitted observation owner.
    pub fn cancel_and_observe(self) -> OwnedJob<T, E> {
        self.job.cancel();
        *self.job
    }

    /// Activate this same pre-held job using its first immutable cleanup deadline.
    /// No new admission, job, cancellation port, clock, byte charge or factory is created.
    ///
    /// # Errors
    /// Rejection preserves this exact permit when its original token, actual held
    /// state, scope, cancellation, deadline or execution clock no longer permits activation.
    pub fn begin(
        self,
        original: CleanupDeadline,
    ) -> Result<OwnedJob<T, E>, SymbolCleanupBeginRejection<T, E>> {
        let decision = self.core.upgrade().ok_or(AdmissionError::ShuttingDown).and_then(|core| {
            let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
            let mut state = lock(&core.state);
            let binding =
                held(&core, &mut state, self.original_job_id(), self.original_scope_id())?;
            if binding.original != Some(original)
                || original.owner_id != core.id
                || original.job_id != self.original_job_id()
                || original.scope_id != self.original_scope_id()
            {
                return Err(AdmissionError::InvalidSymbolAdmission);
            }
            if now >= original.deadline {
                return Err(AdmissionError::DeadlineExpired { deadline: original.deadline, now });
            }
            let index = state
                .queue
                .iter()
                .position(|queued| queued.context.job_id() == original.job_id)
                .ok_or(AdmissionError::InvalidSymbolAdmission)?;
            let record =
                state.jobs.get(&original.job_id).ok_or(AdmissionError::InvalidSymbolAdmission)?;
            let cancel = record.job.cancellation();
            let queued = &mut state.queue[index];
            let activated = JobContext::new(
                core.id,
                original.scope_id,
                queued.context.spec(),
                Some(original.deadline),
                cancel,
                core.clock.clone(),
                AdmittedJobResources::new(queued.context.memory().clone(), None),
            );
            let previous = std::mem::replace(&mut queued.context, activated);
            // Move the whole queued owner without dropping its captured factory under Core.
            let queued = state.queue.remove(index).ok_or(AdmissionError::InvalidSymbolAdmission)?;
            let work = match queued.work {
                Work::BlockingHeld(factory) => Work::Blocking(factory),
                work => work,
            };
            state.queue.insert(index, QueuedJob { context: queued.context, job: queued.job, work });
            if let Some(record) = state.jobs.get_mut(&original.job_id) {
                record.deadline = Some(original.deadline);
            }
            drop(state);
            drop(previous);
            core.changed();
            Ok(())
        });
        match decision {
            Ok(()) => Ok(*self.job),
            Err(error) => Err(SymbolCleanupBeginRejection { error, permit: self }),
        }
    }
}

#[cfg(test)]
#[path = "symbol_cleanup_tests.rs"]
mod tests;
