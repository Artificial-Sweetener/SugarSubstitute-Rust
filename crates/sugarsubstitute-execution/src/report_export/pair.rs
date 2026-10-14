// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate the two finite export roles through one authoritative worker dispatch.
use crate::{
    ExecutionClient,
    admission::{Core, Record, State},
    task::{ErasedJob, TaskFactory},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, JobId, JobPhase, JobSpec, Moment, OwnedJob, Scope,
};

#[derive(Clone, Copy)]
pub(crate) struct Association {
    pub(crate) root: JobId,
    pub(crate) partner: JobId,
    pub(crate) worker: Option<std::thread::ThreadId>,
}

pub(crate) struct Work {
    pub(crate) context: JobContext,
    pub(crate) job: Arc<dyn ErasedJob>,
    pub(crate) factory: TaskFactory,
}

/// Check the actual partner and worker; current partner cancellation caps both roles.
pub(crate) fn live(
    core: &Core,
    state: &State,
    record: &Record,
) -> Result<Option<Moment>, AdmissionError> {
    let Some(pair) = record.report_pair else {
        return Ok(None);
    };
    let invalid = || AdmissionError::InvalidReportExportAdmission;
    let partner = state.jobs.get(&pair.partner).ok_or_else(invalid)?;
    let reciprocal = partner.report_pair.ok_or_else(invalid)?;
    if record.phase != JobPhase::Running
        || partner.phase != JobPhase::Running
        || record.retiring
        || partner.retiring
        || reciprocal.root != pair.root
        || reciprocal.partner != record.spec.id
        || pair.worker != Some(std::thread::current().id())
        || reciprocal.worker != pair.worker
    {
        return Err(invalid());
    }
    crate::admission_liveness::scope_only_deadline(core, state, partner)
}

/// Borrow the two original fixed requests and their genuine pre-held cleanup controls.
/// Construction collects metadata; only `submit_report_export_pair` grants admission.
pub struct ReportExportPairRequest<
    'a,
    ST: Send + 'static,
    SE: Send + 'static,
    RT: Send + 'static,
    RE: Send + 'static,
> {
    source_scope: &'a Scope,
    source_spec: JobSpec,
    source_cleanup: &'a crate::SymbolCleanupPermit<ST, SE>,
    receiving_scope: &'a Scope,
    receiving_spec: JobSpec,
    receiving_cleanup: &'a super::ReportExportCleanupPermit<RT, RE>,
}
impl<'a, ST: Send + 'static, SE: Send + 'static, RT: Send + 'static, RE: Send + 'static>
    ReportExportPairRequest<'a, ST, SE, RT, RE>
{
    /// Borrow exact caller inputs without moving factories, permits or allocating storage.
    pub const fn new(
        source_scope: &'a Scope,
        source_spec: JobSpec,
        source_cleanup: &'a crate::SymbolCleanupPermit<ST, SE>,
        receiving_scope: &'a Scope,
        receiving_spec: JobSpec,
        receiving_cleanup: &'a super::ReportExportCleanupPermit<RT, RE>,
    ) -> Self {
        Self {
            source_scope,
            source_spec,
            source_cleanup,
            receiving_scope,
            receiving_spec,
            receiving_cleanup,
        }
    }
}
/// Own both distinct original logical job observations, in source/receiver order.
pub type ReportExportPairJobs<E> = (OwnedJob<bool, E>, OwnedJob<bool, E>);
/// Return the two genuine observations or the identical unexecuted factory.
pub type ReportExportPairSubmission<E, M> =
    Result<ReportExportPairJobs<E>, super::ReportExportSubmissionRejection<M>>;
pub(crate) struct MainRequest<'a> {
    pub(crate) scope: &'a Scope,
    pub(crate) spec: JobSpec,
    pub(crate) cleanup: JobId,
}

impl ExecutionClient {
    /// Atomically admit the committed-reader and report receiver on one blocking worker.
    /// Both original finite budgets, two queue credits and two terminal credits are charged.
    /// The callback runs synchronously with distinct genuine contexts and original equal
    /// cutoffs; neither main terminates while the callback owns native streaming effects.
    /// Each role retains its own previously admitted independent close-only cleanup.
    /// `E` is copied into the two outcomes; owned causes belong in the charged input owner.
    /// # Errors
    /// Preserves the exact unexecuted factory on foreign/unpaired controls, mismatched
    /// specifications/scopes/cutoffs, overload, shutdown, cancellation or clock failure.
    pub fn submit_report_export_pair<E, M, ST, SE, RT, RE>(
        &self,
        request: ReportExportPairRequest<'_, ST, SE, RT, RE>,
        factory: M,
    ) -> ReportExportPairSubmission<E, M>
    where
        E: Copy + Send + 'static,
        ST: Send + 'static,
        SE: Send + 'static,
        RT: Send + 'static,
        RE: Send + 'static,
        M: FnOnce(JobContext, JobContext) -> Result<bool, E> + Send + 'static,
    {
        let ReportExportPairRequest {
            source_scope,
            source_spec,
            source_cleanup,
            receiving_scope,
            receiving_spec,
            receiving_cleanup,
        } = request;
        if !source_cleanup.belongs_to(&self.core) || !receiving_cleanup.belongs_to(&self.core) {
            return Err(super::ReportExportSubmissionRejection {
                error: AdmissionError::InvalidReportExportAdmission,
                factory,
            });
        }
        self.core
            .admit_report_export_pair(
                MainRequest {
                    scope: source_scope,
                    spec: source_spec,
                    cleanup: source_cleanup.original_job_id(),
                },
                MainRequest {
                    scope: receiving_scope,
                    spec: receiving_spec,
                    cleanup: receiving_cleanup.original_job_id(),
                },
                factory,
            )
            .map_err(|(error, factory)| super::ReportExportSubmissionRejection { error, factory })
    }
}
