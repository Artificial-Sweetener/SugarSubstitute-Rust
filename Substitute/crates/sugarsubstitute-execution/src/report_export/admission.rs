// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate and submit genuine finite export work through the existing central pool.
use super::{
    cleanup::ReportExportCleanupPermit,
    origin::{OriginState, ReportExportAdmission, ReportExportOrigin},
    resources::{Counters, ResourceRecord},
};
use crate::{
    ExecutionClient,
    admission::{Core, RequestKind, State},
    scope::lock,
    task::{Job, JobHandle, QueuedJob, Work},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, CancelControl, JobClass, JobContext, JobPhase, JobSpec, Moment, Operation,
    OwnedJob, Scope,
};
/// Preserve an unexecuted factory and its exact ownership on central admission rejection.
pub struct ReportExportSubmissionRejection<M> {
    /// Genuine central admission failure.
    pub error: AdmissionError,
    /// Identical submitted factory; rejection performs no native effect.
    pub factory: M,
}
pub(crate) fn valid_main(spec: JobSpec) -> bool {
    spec.operation == Operation::Infrastructure
        && spec.class == JobClass::InfrastructureOrdinary
        && spec.reservation_bytes == super::REPORT_EXPORT_BYTES
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
    _core: &Core,
    state: &State,
    scope: &Scope,
    spec: JobSpec,
    kind: &RequestKind,
    now: Moment,
) -> Result<(), AdmissionError> {
    match kind {
        RequestKind::ReportExportCleanup { main_scope, main_spec } => {
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
                || spec.reservation_bytes != super::REPORT_EXPORT_CLEANUP_BYTES
                || spec.deadline.is_some()
                || spec.replacement.is_some()
                || spec.id >= main_spec.id
            {
                return Err(AdmissionError::InvalidReportExportAdmission);
            }
            if let Some(deadline) = main_spec.deadline.filter(|deadline| *deadline <= now) {
                return Err(AdmissionError::DeadlineExpired { deadline, now });
            }
            if state.jobs.contains_key(&main_spec.id)
                || state
                    .jobs
                    .values()
                    .filter_map(|record| record.report_cleanup.as_ref())
                    .any(|binding| binding.main_spec.id == main_spec.id)
            {
                return Err(AdmissionError::DuplicateJobId { job_id: main_spec.id });
            }
        }
        RequestKind::ReportExportMain { cleanup } => {
            let held =
                state.jobs.get(cleanup).ok_or(AdmissionError::InvalidReportExportAdmission)?;
            let binding =
                held.report_cleanup.as_ref().ok_or(AdmissionError::InvalidReportExportAdmission)?;
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
                return Err(AdmissionError::InvalidReportExportAdmission);
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn effect(
    core: &Core,
    state: &State,
    observed: &JobContext,
    counters: &Arc<Counters>,
    now: Moment,
) -> Result<(), AdmissionError> {
    crate::capture_bundle::validate::live(core, state, observed, now)?;
    let record = crate::capture_bundle::validate::association(core, state, observed)?;
    let resource = state
        .report_resources
        .get(&observed.job_id())
        .ok_or(AdmissionError::InvalidReportExportAdmission)?;
    if !valid_main(record.spec)
        || !record.report_export
        || !record.report_export_issued
        || resource.scope_id != observed.scope_id()
        || !resource.issued
        || !resource.matches(counters)
        || observed.native_handles().is_some()
        || !state.jobs.values().any(|record| {
            record.report_cleanup.as_ref().is_some_and(|binding| {
                binding.paired
                    && binding.main_spec == observed.spec()
                    && binding.main_scope.id == observed.scope_id()
            })
        })
    {
        return Err(AdmissionError::InvalidReportExportAdmission);
    }
    Ok(())
}

impl ExecutionClient {
    /// Issue the sole original 128 KiB/68-output export owner from an actual running main.
    /// Matching caller ports are discarded; exact actual memory and Core ports are retained.
    /// # Errors
    /// Rejects foreign/counterfeit memory or native ledgers, unpaired/retired jobs, prior
    /// issuance, current controls/deadlines, exhausted original memory and clock failure.
    pub fn report_export_admission(
        &self,
        observed: &JobContext,
    ) -> Result<ReportExportAdmission, AdmissionError> {
        if ReportExportOrigin::storage_bytes() > 4096 || observed.native_handles().is_some() {
            return Err(AdmissionError::InvalidReportExportAdmission);
        }
        {
            let state = lock(&self.core.state);
            crate::capture_bundle::validate::association(&self.core, &state, observed)?;
        }
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let memory;
        let original;
        let counters;
        {
            let mut state = lock(&self.core.state);
            crate::capture_bundle::validate::live(&self.core, &state, observed, now)?;
            let record =
                crate::capture_bundle::validate::association(&self.core, &state, observed)?;
            if !valid_main(record.spec)
                || !record.report_export
                || record.report_export_issued
                || !state.jobs.values().any(|record| {
                    record.report_cleanup.as_ref().is_some_and(|binding| {
                        binding.paired
                            && binding.main_spec == observed.spec()
                            && binding.main_scope.id == observed.scope_id()
                    })
                })
            {
                return Err(AdmissionError::InvalidReportExportAdmission);
            }
            memory = observed
                .memory()
                .claim(super::REPORT_EXPORT_BYTES)
                .map_err(AdmissionError::Memory)?;
            original = crate::capture_bundle::validate::reconstruct(&self.core, record, observed);
            counters = Arc::new(Counters::new());
            state.report_resources.insert(
                observed.job_id(),
                ResourceRecord {
                    scope_id: observed.scope_id(),
                    issued: true,
                    observation: Arc::downgrade(&counters),
                },
            );
            if let Some(record) = state.jobs.get_mut(&observed.job_id()) {
                record.report_export_issued = true;
            }
        }
        Ok(ReportExportAdmission {
            origin: ReportExportOrigin(Some(Arc::new(OriginState {
                core: self.core.clone(),
                original,
                counters,
                _memory: memory,
            }))),
        })
    }
}

impl ExecutionClient {
    /// Admit fixed finite report export work only after its actual independent cleanup job is held.
    ///
    /// # Errors
    /// Rejects invalid fixed specifications, foreign or unpaired cleanup, and actual
    /// capacity/cancellation/deadline failures, returning the exact unexecuted factory.
    pub fn submit_report_export<T, E, M, CT, CE>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        cleanup: &ReportExportCleanupPermit<CT, CE>,
        factory: M,
    ) -> Result<OwnedJob<T, E>, ReportExportSubmissionRejection<M>>
    where
        T: Send + 'static,
        E: Send + 'static,
        CT: Send + 'static,
        CE: Send + 'static,
        M: FnOnce(JobContext) -> Result<T, E> + Send + 'static,
    {
        if !cleanup.belongs_to(&self.core) {
            return Err(ReportExportSubmissionRejection {
                error: AdmissionError::InvalidReportExportAdmission,
                factory,
            });
        }
        self.core
            .admit_owned(
                scope,
                spec,
                RequestKind::ReportExportMain { cleanup: cleanup.original_job_id() },
                factory,
                |factory, context, cancel, timing| {
                    let admitted = timing.admitted;
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
                    Ok((queued, OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job)))))
                },
            )
            .map_err(|(error, factory)| ReportExportSubmissionRejection { error, factory })
    }
}
