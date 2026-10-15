// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe exact central worker and independent checked file cleanup before projection.
use super::*;
use crate::job_sequence::JobSequence;
use sugarsubstitute_execution::ExecutionClient;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum RecoveryIssue {
    Started,
    Busy,
    Identity,
    Admission,
    Failed,
}
impl std::fmt::Display for RecoveryIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Incident recovery rejected: {self:?}")
    }
}
impl std::error::Error for RecoveryIssue {}
struct Delivery {
    job: Option<OwnedJob<bool, RecoveryIssue>>,
    completion: Option<JobCompletion<bool, RecoveryIssue>>,
    notice: Option<TerminalKind>,
}
impl Delivery {
    const fn new() -> Self {
        Self { job: None, completion: None, notice: None }
    }
    fn capture(&mut self, batch: &CompletionBatch) -> Result<(), RecoveryIssue> {
        let Some(job) = &mut self.job else {
            return Ok(());
        };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                if self.notice.is_some_and(|old| old != notice.kind) {
                    return Err(RecoveryIssue::Identity);
                }
                self.notice = Some(notice.kind);
            }
        }
        if self.notice.is_some()
            && let Some(done) = job.try_take_completion()
        {
            let exact = done.scope_id == job.scope_id()
                && done.spec == job.spec()
                && self.notice == Some(done.outcome.kind());
            self.completion = Some(done);
            self.job = None;
            if !exact {
                return Err(RecoveryIssue::Identity);
            }
        }
        Ok(())
    }
    fn success(&self) -> bool {
        self.completion
            .as_ref()
            .is_some_and(|done| matches!(done.outcome, JobOutcome::Success(true)))
    }
    fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
}
pub(in crate::production) struct RecoveryService {
    source: source::Source,
    work: Delivery,
    cleanup: Delivery,
    requested: Option<(ExecutionId, ScopeId, Moment)>,
    cleanup_requested: Option<(ScopeId, Moment)>,
    installed: bool,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    adopted: bool,
    failure: Option<RecoveryFailure>,
}
impl RecoveryService {
    pub(in crate::production) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        Ok(Self {
            source: source::Source::new(context)?,
            work: Delivery::new(),
            cleanup: Delivery::new(),
            requested: None,
            cleanup_requested: None,
            installed: false,
            admission: None,
            identity: None,
            adopted: false,
            failure: None,
        })
    }
    pub(in crate::production) fn submit(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
        input: &mut Option<RecoveryInput>,
    ) -> Result<(), RecoveryIssue> {
        if !self.installed {
            self.source.install(input)?;
            self.installed = true;
        }
        if self.work.job.is_some() || self.work.completion.is_some() {
            return Err(RecoveryIssue::Started);
        }
        let requested = (scope.owner_id(), scope.id(), until);
        if requested.0 != self.source.owner() || self.requested.is_some_and(|old| old != requested)
        {
            return Err(RecoveryIssue::Identity);
        }
        self.requested = Some(requested);
        let source = self.source.alias();
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            RecoveryIssue::Identity
        })?;
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            let mut loan = source.begin()?;
            let operation = loan.operation()?;
            let input = operation.input.take().ok_or(RecoveryIssue::Identity)?;
            operation.execute(input, &context);
            Ok(true)
        }) {
            Ok(job) => {
                self.work.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(RecoveryIssue::Admission)
            }
        }
    }
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(), RecoveryIssue> {
        self.work.capture(batch)?;
        self.cleanup.capture(batch)?;
        let _retained_failure = self.retain_failure();
        Ok(())
    }
    pub(in crate::production) fn delivered(&self) -> bool {
        self.work.completion.is_some()
    }
    pub(in crate::production) fn cleanup_started(&self) -> bool {
        self.cleanup.job.is_some() || self.cleanup.completion.is_some()
    }
    pub(in crate::production) fn submit_cleanup(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), RecoveryIssue> {
        if !self.delivered() {
            return Err(RecoveryIssue::Busy);
        }
        if scope.owner_id() != self.source.owner() {
            return Err(RecoveryIssue::Identity);
        }
        let requested = (scope.id(), until);
        if self.cleanup_requested.is_some_and(|old| old != requested) {
            return Err(RecoveryIssue::Identity);
        }
        if self.cleanup.job.is_some() || self.cleanup.completion.is_some() {
            return Ok(());
        }
        self.cleanup_requested = Some(requested);
        let source = self.source.alias();
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            RecoveryIssue::Identity
        })?;
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: 4096,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            let mut loan = source.loan()?;
            Ok(loan.operation()?.retire(&context))
        }) {
            Ok(job) => {
                self.cleanup_requested = Some(requested);
                self.cleanup.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(RecoveryIssue::Admission)
            }
        }
    }
    pub(in crate::production) fn files_retired(&self) -> bool {
        self.cleanup.success() && self.source.inspect(|operation| operation.retired) == Some(true)
    }
    pub(in crate::production) fn pending(&self) -> bool {
        self.installed && !self.adopted || self.work.job.is_some() || self.cleanup.job.is_some()
    }
    pub(in crate::production) fn cleanup_stopped(&self) -> bool {
        self.cleanup.completion.is_some() && !self.cleanup.success()
    }
    /// No admitted job/terminal/native work may exist; retained rejection and exact input survive.
    pub(in crate::production) fn restore_unsubmitted(
        &mut self,
        output: &mut Option<RecoveryInput>,
    ) -> Result<bool, RecoveryIssue> {
        if self.work.job.is_some()
            || self.work.completion.is_some()
            || self.cleanup_requested.is_some()
        {
            return Ok(false);
        }
        self.source.restore_unstarted(output)?;
        self.adopted = true;
        Ok(true)
    }
    pub(in crate::production) fn cancel(&self) {
        self.work.cancel();
        self.cleanup.cancel();
    }
    /// Adopt completed Main and cleanup causes independently of actual native settlement.
    /// Moving owned errors and cloning their genuine reservation invokes no provider or allocator.
    pub(in crate::production) fn retain_failure(&mut self) -> bool {
        if self.work.completion.is_some() {
            if self.failure.is_none() {
                self.failure = self.source.take_failure();
            }
            if let Some(failure) = &mut self.failure
                && failure.retirement.is_none()
            {
                failure.retirement = self.source.take_retirement_failure(&failure._original);
            }
        }
        self.failure.is_some() || self.work.completion.is_some() && !self.work.success()
    }
    pub(in crate::production) fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.failure
            .as_ref()
            .map(|cause| cause as _)
            .or_else(|| self.admission.as_ref().map(|cause| cause as _))
            .or_else(|| self.identity.as_ref().map(|cause| cause as _))
    }
    pub(in crate::production) fn inspect<T>(
        &self,
        f: impl FnOnce(&PendingIncidentSnapshot) -> T,
    ) -> Result<T, RecoveryIssue> {
        if self.failure.is_some() || !self.work.success() || !self.files_retired() {
            return Err(RecoveryIssue::Busy);
        }
        self.source
            .inspect(|operation| {
                if operation.failed() {
                    return Err(RecoveryIssue::Failed);
                }
                operation.pending.as_ref().map(f).ok_or(RecoveryIssue::Failed)
            })
            .ok_or(RecoveryIssue::Busy)?
    }
    pub(in crate::production) fn release_adopted(&mut self) -> Result<(), RecoveryIssue> {
        if self.failure.is_some() {
            return Err(RecoveryIssue::Failed);
        }
        self.source.release()?;
        self.work.completion = None;
        self.cleanup.completion = None;
        self.adopted = true;
        Ok(())
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
