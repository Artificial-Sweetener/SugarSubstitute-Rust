// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit and observe actual finite collection and independent close-only worker lifetimes.
use super::{CollectionInput, CollectionOperation, source::Source};
use crate::{job_sequence::JobSequence, production::incident_host::FundedState};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionClient;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum CollectionIssue {
    Busy,
    Identity,
    Admission,
}

impl std::fmt::Display for CollectionIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Incident collection rejected: {self:?}")
    }
}
impl std::error::Error for CollectionIssue {}
pub(in crate::production) struct CollectionService {
    source: Source,
    job: Option<OwnedJob<(), CollectionIssue>>,
    completion: Option<JobCompletion<(), CollectionIssue>>,
    terminal: Option<TerminalKind>,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    requested: Option<(ExecutionId, ScopeId, Moment)>,
    installed: bool,
    passive_restored: Option<sugarsubstitute_application::incident::IncidentOperationScope>,
    passive_observation_pending: bool,
    cleanup: Option<OwnedJob<bool, CollectionIssue>>,
    cleanup_completion: Option<JobCompletion<bool, CollectionIssue>>,
    cleanup_terminal: Option<TerminalKind>,
    cleanup_binding: Option<(ScopeId, Moment)>,
}
impl CollectionService {
    /// Restore the identical controller after actual central admission rejected all work.
    /// No job, completion or native cleanup is manufactured; original causes remain here.
    pub(in crate::production) fn abandon_unsubmitted(
        &mut self,
        policy: &mut Option<FundedState>,
    ) -> Result<sugarsubstitute_application::incident::IncidentOperationScope, CollectionIssue>
    {
        if !self.installed
            || self.job.is_some()
            || self.completion.is_some()
            || self.cleanup.is_some()
            || self.cleanup_completion.is_some()
        {
            return Err(CollectionIssue::Busy);
        }
        let scope = self.source.abandon_unsubmitted(policy)?;
        self.passive_restored = Some(scope);
        self.passive_observation_pending = true;
        self.installed = false;
        Ok(scope)
    }
    /// Observe only the authoritative source's actual retained or issued original policy.
    pub(in crate::production) fn policy_pending(&self) -> bool {
        self.source.policy_pending()
    }
    /// Borrow original correlation only after passive custody was actually restored outside locks.
    /// This applies to central rejection and genuine terminal delivery before Source::begin.
    pub(in crate::production) const fn passive_restored_scope(
        &self,
    ) -> Option<sugarsubstitute_application::incident::IncidentOperationScope> {
        self.passive_restored
    }
    /// Offer the original passive scope once for the funded coordinator's pure Failed observation.
    /// Consuming logical delivery keeps the independently checked custody scope observable.
    pub(in crate::production) fn take_passive_restoration_observation(
        &mut self,
    ) -> Option<sugarsubstitute_application::incident::IncidentOperationScope> {
        if !self.passive_observation_pending {
            return None;
        }
        self.passive_observation_pending = false;
        self.passive_restored
    }
    /// Borrow the unchanged actual rejected admission/identity cause after passive restoration.
    pub(in crate::production) fn unsubmitted_failure(
        &self,
    ) -> Option<&(dyn std::error::Error + 'static)> {
        self.admission
            .as_ref()
            .map(|cause| cause as _)
            .or_else(|| self.identity.as_ref().map(|cause| cause as _))
    }
    /// Observe actual delivered original Core terminal, independently from checked cleanup.
    pub(in crate::production) fn outcome(&self) -> Option<TerminalKind> {
        self.completion.as_ref().map(|done| done.outcome.kind())
    }
    /// Observe actual delivered cleanup terminal; Success alone does not establish file closure.
    pub(in crate::production) fn cleanup_outcome(&self) -> Option<TerminalKind> {
        self.cleanup_completion.as_ref().map(|done| done.outcome.kind())
    }
    /// Preserve and categorize the original cause/product after independently checked cleanup.
    pub(in crate::production) fn failed(&self) -> bool {
        self.outcome().is_some_and(|kind| kind != TerminalKind::Success)
            || self.source.inspect(|operation| !operation.releasable()) == Some(true)
    }
    pub(in crate::production) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        Ok(Self {
            source: Source::new(context)?,
            job: None,
            completion: None,
            terminal: None,
            admission: None,
            identity: None,
            requested: None,
            installed: false,
            passive_restored: None,
            passive_observation_pending: false,
            cleanup: None,
            cleanup_completion: None,
            cleanup_terminal: None,
            cleanup_binding: None,
        })
    }
    pub(in crate::production) fn submit(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
        input: &mut Option<CollectionInput>,
        policy: &mut Option<FundedState>,
    ) -> Result<(), CollectionIssue> {
        if self.installed
            || self.passive_restored.is_some()
            || scope.owner_id() != self.source.owner()
            || input.as_ref().is_none_or(|input| until > input.scope.original().until())
        {
            return Err(CollectionIssue::Identity);
        }
        self.source.install(input, policy)?;
        self.installed = true;
        self.retry(client, scope, jobs, until)
    }
    pub(in crate::production) fn retry(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), CollectionIssue> {
        let requested = (scope.owner_id(), scope.id(), until);
        if !self.installed
            || self.job.is_some()
            || self.completion.is_some()
            || scope.owner_id() != self.source.owner()
            || self.requested.is_some_and(|old| old != requested)
        {
            return Err(CollectionIssue::Busy);
        }
        self.requested = Some(requested);
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            CollectionIssue::Identity
        })?;
        let source = self.source.alias();
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: 8 * 1024 * 1024,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            let (mut loan, input) = source.begin()?;
            loan.execute(input, &context)
        }) {
            Ok(job) => {
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(CollectionIssue::Admission)
            }
        }
    }
    pub(in crate::production) fn submit_cleanup(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), CollectionIssue> {
        if self.job.is_some()
            || self.completion.is_none()
            || self.passive_restored.is_some()
            || self.cleanup.is_some()
            || self.cleanup_completion.is_some()
            || scope.owner_id() != self.source.owner()
            || self.cleanup_binding.is_some_and(|old| old != (scope.id(), until))
        {
            return Err(CollectionIssue::Busy);
        }
        self.cleanup_binding = Some((scope.id(), until));
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            CollectionIssue::Identity
        })?;
        let source = self.source.alias();
        let execution = client.clone();
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: 32768,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            let mut loan = source.loan()?;
            loan.retire(&execution, &context)
        }) {
            Ok(job) => {
                self.cleanup = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(CollectionIssue::Admission)
            }
        }
    }
    pub(in crate::production) fn capture_cleanup(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, CollectionIssue> {
        let Some(job) = &mut self.cleanup else { return Ok(false) };
        let mut matched = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.cleanup_terminal.is_some_and(|old| old != notice.kind) {
                return Err(CollectionIssue::Identity);
            }
            self.cleanup_terminal = Some(notice.kind);
            matched = true;
        }
        if self.cleanup_terminal.is_some()
            && let Some(completion) = job.try_take_completion()
        {
            let valid = completion.scope_id == job.scope_id()
                && completion.spec == job.spec()
                && self.cleanup_terminal == Some(completion.outcome.kind());
            self.cleanup_completion = Some(completion);
            self.cleanup = None;
            if !valid {
                return Err(CollectionIssue::Identity);
            }
        }
        Ok(matched)
    }
    pub(in crate::production) fn files_retired(&self) -> bool {
        matches!(
            self.cleanup_completion.as_ref().map(|done| &done.outcome),
            Some(JobOutcome::Success(true))
        ) && self.source.inspect(CollectionOperation::files_retired) == Some(true)
    }
    pub(in crate::production) fn completed_scope(
        &self,
    ) -> Option<sugarsubstitute_application::incident::IncidentOperationScope> {
        if !self.files_retired() {
            return None;
        }
        self.source.inspect(CollectionOperation::scope).flatten()
    }
    pub(in crate::production) fn release_adopted(&mut self) -> Result<(), CollectionIssue> {
        if !self.files_retired() {
            return Err(CollectionIssue::Busy);
        }
        self.source.release_retired()?;
        self.completion = None;
        self.cleanup_completion = None;
        self.terminal = None;
        self.cleanup_terminal = None;
        self.requested = None;
        self.cleanup_binding = None;
        self.installed = false;
        self.passive_restored = None;
        self.passive_observation_pending = false;
        Ok(())
    }
    pub(in crate::production) fn inspect<T>(
        &self,
        read: impl FnOnce(&CollectionOperation) -> T,
    ) -> Option<T> {
        self.source.inspect(read)
    }
    pub(in crate::production) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "service_probe.rs"]
mod policy_probe;

#[path = "service_completion.rs"]
mod completion;
