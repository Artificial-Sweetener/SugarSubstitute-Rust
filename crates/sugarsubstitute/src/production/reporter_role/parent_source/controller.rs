// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match every original finite relay admission and preserve source custody through actual delivery.
use super::*;
#[path = "controller/recycling.rs"]
mod recycling;
use crate::job_sequence::JobSequence;
/// Native callers submit and observe finite Core work without waiting or borrowing the router.
pub(in crate::production) struct ReporterRelay {
    source: ReporterSource,
    job: Option<OwnedJob<(), ReporterSourceIssue>>,
    completion: Option<JobCompletion<(), ReporterSourceIssue>>,
    terminal: Option<TerminalKind>,
    matched: bool,
    original: Option<(ExecutionId, ScopeId, Moment)>,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
}
impl ReporterRelay {
    /// Retain the already funded source; no metadata grant or queue is created here.
    pub(in crate::production) fn new(source: ReporterSource) -> Self {
        Self {
            source,
            job: None,
            completion: None,
            terminal: None,
            matched: false,
            original: None,
            admission: None,
            identity: None,
        }
    }
    /// Borrow native fact/launch methods; wire methods remain private to finite Core processing.
    pub(in crate::production) fn source(&self) -> &ReporterSource {
        &self.source
    }
    /// Latch the first queue cutoff; retries cannot move it or the original scope forward.
    pub(in crate::production) fn submit(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), ReporterSourceIssue> {
        if self.job.is_some() || self.completion.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        let until = self.source.cutoff(until)?;
        let current = (scope.owner_id(), scope.id(), until);
        if self.original.is_some_and(|original| original != current) {
            return Err(ReporterSourceIssue::Identity);
        }
        self.original = Some(current);
        self.matched = false;
        self.retry(client, scope, jobs)
    }
    /// Retry only the same original central admission, retaining its exact input/deadline.
    pub(in crate::production) fn retry(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), ReporterSourceIssue> {
        if self.job.is_some() || self.completion.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        let (owner, expected_scope, until) = self.original.ok_or(ReporterSourceIssue::Identity)?;
        if (owner, expected_scope) != (scope.owner_id(), scope.id()) {
            return Err(ReporterSourceIssue::Identity);
        }
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            ReporterSourceIssue::Identity
        })?;
        let source = self.source.alias();
        let execution = client.clone();
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: worker::REPORTER_CORE_BYTES,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            source.process(
                &context,
                || execution.now(),
                |until| execution.encode_counter_deadline(until),
            )
        }) {
            Ok(job) => {
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(ReporterSourceIssue::Admission)
            }
        }
    }
    /// Match actual notices and exact completion; a foreign or conflicting terminal grants nothing.
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, ReporterSourceIssue> {
        let Some(job) = &mut self.job else { return Ok(false) };
        let mut matched = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|old| old != notice.kind) {
                return Err(ReporterSourceIssue::Identity);
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
            self.matched = valid;
            if !valid {
                return Err(ReporterSourceIssue::Identity);
            }
        }
        Ok(matched)
    }
    /// Release output only after actual matched delivery; rejected completion tokens also return.
    /// Failures retain their completion, source, partial view and original causes for checked cleanup.
    pub(in crate::production) fn take_output(
        &mut self,
    ) -> Result<Option<ReporterOutput>, ReporterSourceIssue> {
        let Some(done) = &self.completion else { return Ok(None) };
        if !self.matched {
            return Err(ReporterSourceIssue::Identity);
        }
        let output = self.source.take_output()?;
        if matches!(done.outcome, JobOutcome::Success(())) {
            self.completion = None;
            self.terminal = None;
            self.original = None;
        }
        Ok(output)
    }
    /// Observe an escaped original panic while the loan has restored the complete funded source.
    pub(in crate::production) fn panic_cause(&self) -> Option<&PanicCause> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Panicked(cause)) => Some(cause),
            _ => None,
        }
    }
    /// Borrow central admission cause without replacing its original metadata.
    pub(in crate::production) fn admission_failure(&self) -> Option<&AdmissionError> {
        self.admission.as_ref()
    }
    /// Borrow the actual original finite worker's failure after its matched terminal delivery.
    pub(in crate::production) fn worker_failure(&self) -> Option<&ReporterSourceIssue> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Failure(cause)) => Some(cause),
            _ => None,
        }
    }
    /// Return only the matched actual terminal, including cancellation/deadline without settlement.
    pub(in crate::production) fn worker_terminal(&self) -> Option<TerminalKind> {
        self.matched.then(|| self.completion.as_ref().map(|done| done.outcome.kind())).flatten()
    }
    /// Actual delivered means only this finite worker has completed, never native-family settlement.
    pub(in crate::production) fn worker_delivered(&self) -> bool {
        self.job.is_none()
    }
    /// Request cancellation without dropping source or claiming native or worker settlement.
    pub(in crate::production) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
}

impl ReporterRelay {
    /// Move unchanged original failure and real Core delivery only after checked native closure.
    pub(in crate::production) fn take_retired_failure(
        &mut self,
        registry: &WindowsProcessRegistry,
    ) -> Result<Option<OwnedReporterFailure>, ReporterSourceIssue> {
        if !self.worker_delivered() {
            return Err(ReporterSourceIssue::Busy);
        }
        let mut failure = self.source.take_retired_failure(registry)?;
        if self.matched
            && self
                .completion
                .as_ref()
                .is_some_and(|done| done.outcome.kind() != TerminalKind::Success)
        {
            let done = self.completion.take().ok_or(ReporterSourceIssue::Identity)?;
            if let Some(failure) = &mut failure {
                failure._worker = Some(done);
            } else {
                self.source.loan()?.state()?.failure_transferred = true;
                failure = Some(OwnedReporterFailure {
                    cause: OriginalCause::Worker(done),
                    _worker: None,
                    _source: self.source.alias(),
                });
            }
        }
        Ok(failure)
    }
}
