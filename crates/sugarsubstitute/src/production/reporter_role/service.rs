// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit complete-view parsing on finite Core jobs while retaining the original source and cutoff.
use super::{
    client::NativeInput,
    source::{ViewSource, ViewSourceIssue},
    view::PreparedView,
};
use crate::job_sequence::JobSequence;
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionClient;
pub(super) struct ViewService {
    source: ViewSource,
    job: Option<OwnedJob<(), ViewSourceIssue>>,
    completion: Option<JobCompletion<(), ViewSourceIssue>>,
    terminal: Option<TerminalKind>,
    original: Option<(ExecutionId, ScopeId, Moment)>,
    admitted_input: bool,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    admission_transferred: bool,
}
impl ViewService {
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        Ok(Self {
            source: ViewSource::new(context)?,
            job: None,
            completion: None,
            terminal: None,
            original: None,
            admitted_input: false,
            admission: None,
            identity: None,
            admission_transferred: false,
        })
    }
    /// Install one original complete input. Denial leaves it unchanged or retained in this owner.
    pub(super) fn submit(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
        input: &mut Option<NativeInput>,
    ) -> Result<(), ViewServiceIssue> {
        if self.admission_transferred
            || self.admitted_input
            || self.job.is_some()
            || self.completion.is_some()
        {
            return Err(ViewServiceIssue::Busy);
        }
        let requested = (scope.owner_id(), scope.id(), until);
        if self.original.is_some_and(|original| original != requested) {
            return Err(ViewServiceIssue::Identity);
        }
        self.source.install(input).map_err(|_| ViewServiceIssue::Busy)?;
        self.original = Some(requested);
        self.admitted_input = true;
        self.retry(client, scope, jobs)
    }
    /// Retry central admission only; never reconstruct an input or renew a transfer deadline.
    pub(super) fn retry(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), ViewServiceIssue> {
        if self.admission_transferred
            || !self.admitted_input
            || self.job.is_some()
            || self.completion.is_some()
        {
            return Err(ViewServiceIssue::Busy);
        }
        let (owner, expected_scope, until) = self.original.ok_or(ViewServiceIssue::Identity)?;
        if (owner, expected_scope) != (scope.owner_id(), scope.id()) {
            return Err(ViewServiceIssue::Identity);
        }
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            ViewServiceIssue::Identity
        })?;
        let source = self.source.alias();
        let execution = client.clone();
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: 64 * 1024,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            source.process(
                &context,
                |deadline| {
                    let original = CounterDeadline::new(deadline.ticks(), deadline.frequency())
                        .map_err(|cause| {
                            sugarsubstitute_execution::windows::NativeObservationError::new(
                                "reporter_wire_deadline",
                                cause,
                            )
                        })?;
                    execution.decode_counter_deadline(original)
                },
                || {
                    execution.now().map_err(|cause| {
                        sugarsubstitute_execution::windows::NativeObservationError::new(
                            "reporter_input_clock",
                            cause,
                        )
                    })
                },
            )
        }) {
            Ok(job) => {
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(ViewServiceIssue::Admission)
            }
        }
    }
    /// Take only the terminal matching this actual original job; foreign notices grant nothing.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<bool, ViewServiceIssue> {
        let Some(job) = &mut self.job else {
            return Ok(false);
        };
        let mut matched = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|old| old != notice.kind) {
                return Err(ViewServiceIssue::Identity);
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
                return Err(ViewServiceIssue::Identity);
            }
        }
        Ok(matched)
    }
    /// Keep the same cutoff across partial chunks. A complete view ends that transfer only.
    pub(super) fn take_prepared(&mut self) -> Result<Option<PreparedView>, ViewServiceIssue> {
        let Some(completion) = &self.completion else {
            return Ok(None);
        };
        if !matches!(completion.outcome, JobOutcome::Success(())) {
            return Err(ViewServiceIssue::Failed);
        }
        let prepared = self.source.take_prepared().ok_or(ViewServiceIssue::Failed)?;
        if matches!(prepared, PreparedView::Partial) {
            if let (Some(original), Some(until)) = (&mut self.original, self.source.input_until()) {
                original.2 = original.2.min(until);
            }
        } else {
            self.original = None;
        }
        self.completion = None;
        self.terminal = None;
        self.admitted_input = false;
        Ok(Some(prepared))
    }
    pub(super) fn presented(
        &self,
        binding: sugarsubstitute_contracts::incident::ReportBinding,
    ) -> Result<(), ViewServiceIssue> {
        self.source.presented(binding).map_err(|_| ViewServiceIssue::Busy)
    }
    /// Reuse the finite cutoff across every partial frame of the current transfer.
    /// A completed transfer releases only that operation's cutoff, not the session.
    pub(super) fn input_until(&self) -> Option<Moment> {
        self.original.map(|original| original.2)
    }
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    /// Borrow the actual escaped original panic; receiving it never proves successful presentation.
    pub(super) fn panic_cause(&self) -> Option<&PanicCause> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Panicked(cause)) => Some(cause),
            _ => None,
        }
    }
    pub(super) fn worker_delivered(&self) -> bool {
        self.job.is_none()
    }
    /// Take the original view failure only after its actual finite worker has delivered.
    pub(super) fn take_failure(&self) -> Option<super::view::OwnedViewFailure> {
        self.worker_delivered().then(|| self.source.take_failure()).flatten()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewServiceIssue {
    Busy,
    Identity,
    Admission,
    Failed,
}

#[path = "service/admission_failure.rs"]
mod admission_failure;
pub(super) use admission_failure::OwnedViewAdmissionFailure;
