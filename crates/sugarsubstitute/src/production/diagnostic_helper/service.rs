// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive one seed worker and independently checked file cleanup on real finite jobs.
use super::{SeedInput, SeedOperation, source::Source};
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::diagnostic_storage::DIAGNOSTIC_STORAGE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum SeedServiceIssue {
    Started,
    Identity,
    Busy,
    Admission,
}
impl fmt::Display for SeedServiceIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Incident seed service rejected: {self:?}")
    }
}
impl Error for SeedServiceIssue {}
/// Keep the funded input, actual partial repository and completion until checked cleanup.
/// This parent-side owner never waits or performs protected storage effects.
pub(in crate::production) struct SeedService {
    source: Source,
    job: Option<OwnedJob<(), SeedServiceIssue>>,
    completion: Option<JobCompletion<(), SeedServiceIssue>>,
    terminal: Option<TerminalKind>,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    installed: bool,
    requested: Option<(ExecutionId, ScopeId, Moment)>,
    cleanup: Option<OwnedJob<bool, SeedServiceIssue>>,
    cleanup_completion: Option<JobCompletion<bool, SeedServiceIssue>>,
    cleanup_terminal: Option<TerminalKind>,
    cleanup_binding: Option<(ScopeId, Moment)>,
}
impl SeedService {
    /// Claim source/control storage inside the original pre-capture controller reservation.
    pub(in crate::production) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        Ok(Self {
            source: Source::new(context)?,
            job: None,
            completion: None,
            terminal: None,
            admission: None,
            identity: None,
            installed: false,
            requested: None,
            cleanup: None,
            cleanup_completion: None,
            cleanup_terminal: None,
            cleanup_binding: None,
        })
    }
    /// Submit the exact retained input under one finite original8MiB scope and deadline.
    /// Admission rejection retains that input here; `retry` submits it without reconstruction.
    pub(in crate::production) fn submit(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
        input: &mut Option<SeedInput>,
    ) -> Result<(), SeedServiceIssue> {
        if self.installed {
            return Err(SeedServiceIssue::Started);
        }
        if scope.owner_id() != self.source.owner() {
            return Err(SeedServiceIssue::Identity);
        }
        self.source.install(input)?;
        self.installed = true;
        self.retry(client, scope, jobs, until)
    }
    /// Retry only an unexecuted retained input after actual central admission rejection.
    pub(in crate::production) fn retry(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), SeedServiceIssue> {
        if !self.installed || self.job.is_some() || self.completion.is_some() {
            return Err(SeedServiceIssue::Started);
        }
        let requested = (scope.owner_id(), scope.id(), until);
        if requested.0 != self.source.owner() {
            return Err(SeedServiceIssue::Identity);
        }
        if self.requested.is_some_and(|old| old != requested) {
            return Err(SeedServiceIssue::Identity);
        }
        self.requested = Some(requested);
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            SeedServiceIssue::Identity
        })?;
        let source = self.source.alias();
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            let (mut loan, input) = source.begin()?;
            loan.operation()?.execute(input, &context);
            Ok(())
        }) {
            Ok(job) => {
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(SeedServiceIssue::Admission)
            }
        }
    }
    /// Match genuine terminal notices before taking the original completion exactly once.
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, SeedServiceIssue> {
        let Some(job) = &mut self.job else {
            return Ok(false);
        };
        let mut matched = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|old| old != notice.kind) {
                return Err(SeedServiceIssue::Identity);
            }
            self.terminal = Some(notice.kind);
            matched = true;
        }
        if self.terminal.is_some()
            && let Some(completion) = job.try_take_completion()
        {
            let valid = completion.scope_id == job.scope_id()
                && completion.spec == job.spec()
                && self.terminal == Some(completion.outcome.kind());
            self.completion = Some(completion);
            self.job = None;
            if !valid {
                return Err(SeedServiceIssue::Identity);
            }
        }
        Ok(matched)
    }
    /// Cancel actual execution; its partial repository stays in the original source cell.
    pub(in crate::production) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    /// Borrow passive committed/failure data only; a running worker leaves the cell unavailable.
    pub(in crate::production) fn inspect<T>(
        &self,
        observe: impl FnOnce(&SeedOperation) -> T,
    ) -> Option<T> {
        self.source.inspect(observe)
    }
    /// Return the actual completed seed's native/data charge after exact policy adoption.
    /// Source metadata remains funded by the pre-capture controller for the next seed.
    pub(in crate::production) fn release_adopted(&mut self) -> Result<(), SeedServiceIssue> {
        if !self.files_retired() || self.outcome() != Some(TerminalKind::Success) {
            return Err(SeedServiceIssue::Busy);
        }
        self.source.release_retired()?;
        self.completion = None;
        self.cleanup_completion = None;
        self.terminal = None;
        self.cleanup_terminal = None;
        self.requested = None;
        self.cleanup_binding = None;
        self.installed = false;
        Ok(())
    }
    /// Observe central delivery separately from file closure and actual worker joins.
    pub(in crate::production) fn outcome(&self) -> Option<TerminalKind> {
        self.completion.as_ref().map(|value| value.outcome.kind())
    }
}

#[path = "service/cleanup.rs"]
mod cleanup;
