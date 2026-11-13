// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match every original finite relay admission and preserve source custody through actual delivery.
use super::*;
#[path = "controller/recycling.rs"]
mod recycling;
use crate::job_sequence::JobSequence;
/// A matched delivery distinguishes absent completion from actual source progress or deferral.
// Keep the existing funded output on the stack; a box would require independent admitted storage.
#[allow(clippy::large_enum_variant)]
pub(in crate::production) enum ReporterDelivery {
    /// No matched successful completion is available; retained failure remains queryable separately.
    Pending,
    /// A matched finite worker returned without borrowing the original source.
    Deferred,
    /// A matched worker inspected the source, with its actual optional output.
    Advanced(Option<ReporterOutput>),
}
const _: () = assert!(
    size_of::<ReporterDelivery>() <= size_of::<Option<ReporterOutput>>() + size_of::<usize>()
);
/// One actual central lifecycle owns either its job, delivered completion, or rejected admission.
enum RelayWork {
    Idle,
    Admitted(OwnedJob<worker::RelayTurn, ReporterSourceIssue>),
    Delivered(JobCompletion<worker::RelayTurn, ReporterSourceIssue>),
    Rejected(AdmissionError),
}
impl RelayWork {
    fn has_failure(&self, matched: bool) -> bool {
        match self {
            Self::Delivered(done) => matched && done.outcome.kind() != TerminalKind::Success,
            Self::Rejected(_) => true,
            Self::Idle | Self::Admitted(_) => false,
        }
    }
    /// Transfer actual Core/admission custody only; this grants no native retirement proof.
    fn take_failure(&mut self, matched: bool) -> Option<failure::ReporterSecondaryFailure> {
        if !self.has_failure(matched) {
            return None;
        }
        match std::mem::replace(self, Self::Idle) {
            Self::Delivered(done) => {
                Some(failure::ReporterSecondaryFailure::Worker { _completion: done })
            }
            Self::Rejected(cause) => {
                Some(failure::ReporterSecondaryFailure::Admission { _cause: cause })
            }
            actual @ (Self::Idle | Self::Admitted(_)) => {
                *self = actual;
                None
            }
        }
    }
}
/// Native callers submit and observe finite Core work without waiting or borrowing the router.
pub(in crate::production) struct ReporterRelay {
    source: ReporterSource,
    work: RelayWork,
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
            work: RelayWork::Idle,
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
        if matches!(self.work, RelayWork::Admitted(_) | RelayWork::Delivered(_)) {
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
    /// Retry only the same original operation after a nonblocking loan-readiness check.
    /// An unavailable loan consumes no job identity/admission; only the actual loan owner can wake progress.
    /// Every accepted retry retains the original execution, scope, input and cutoff.
    pub(in crate::production) fn retry(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), ReporterSourceIssue> {
        if matches!(self.work, RelayWork::Admitted(_) | RelayWork::Delivered(_)) {
            return Err(ReporterSourceIssue::Busy);
        }
        if let RelayWork::Rejected(cause) = &self.work {
            return Err(if matches!(cause, AdmissionError::DeadlineExpired { .. }) {
                ReporterSourceIssue::Deadline
            } else {
                ReporterSourceIssue::Admission
            });
        }
        let (owner, expected_scope, until) = self.original.ok_or(ReporterSourceIssue::Identity)?;
        if (owner, expected_scope) != (scope.owner_id(), scope.id()) {
            return Err(ReporterSourceIssue::Identity);
        }
        if self.source.cutoff(until)? != until {
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
                self.work = RelayWork::Admitted(job);
                Ok(())
            }
            Err(cause) => {
                if Self::capacity_rejection(&cause) {
                    self.admission.get_or_insert(cause);
                    return Err(ReporterSourceIssue::Admission);
                }
                let issue = if matches!(cause, AdmissionError::DeadlineExpired { .. }) {
                    ReporterSourceIssue::Deadline
                } else {
                    ReporterSourceIssue::Admission
                };
                self.work = RelayWork::Rejected(cause);
                Err(issue)
            }
        }
    }
    /// Match actual notices and exact completion; a foreign or conflicting terminal grants nothing.
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, ReporterSourceIssue> {
        let RelayWork::Admitted(job) = &mut self.work else { return Ok(false) };
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
            self.work = RelayWork::Delivered(completion);
            self.matched = valid;
            if !valid {
                return Err(ReporterSourceIssue::Identity);
            }
        }
        Ok(matched)
    }
    /// Consume only matched success; deferral keeps the original operation and untouched source.
    /// Failed completions and partial output remain retained for checked native cleanup.
    pub(in crate::production) fn take_delivery(
        &mut self,
    ) -> Result<ReporterDelivery, ReporterSourceIssue> {
        let RelayWork::Delivered(done) = &self.work else { return Ok(ReporterDelivery::Pending) };
        if !self.matched {
            return Err(ReporterSourceIssue::Identity);
        }
        let delivery = match &done.outcome {
            JobOutcome::Success(worker::RelayTurn::Deferred) => ReporterDelivery::Deferred,
            JobOutcome::Success(worker::RelayTurn::Advanced) => {
                ReporterDelivery::Advanced(self.source.take_output()?)
            }
            JobOutcome::Failure(_)
            | JobOutcome::Panicked(_)
            | JobOutcome::Cancelled
            | JobOutcome::TimedOut
            | JobOutcome::Coalesced { .. } => {
                return Ok(ReporterDelivery::Pending);
            }
        };
        if matches!(delivery, ReporterDelivery::Advanced(_)) {
            self.original = None;
        }
        self.work = RelayWork::Idle;
        self.terminal = None;
        self.matched = false;
        Ok(delivery)
    }
    /// A genuine uncompleted original operation retries with its unchanged scope and cutoff.
    pub(in crate::production) fn retry_pending(&self) -> bool {
        self.original.is_some() && matches!(self.work, RelayWork::Idle | RelayWork::Rejected(_))
    }
    /// Original finite cutoff in this execution owner's nanoseconds, including deferred admission.
    pub(in crate::production) fn deadline(&self) -> Option<Moment> {
        self.original.map(|(_, _, until)| until)
    }
    /// Enforce a deferred/rejected retry cutoff even while another owner holds its source loan.
    /// Running work and actual nondeferred completions keep their central terminal authority.
    /// The caller samples this same execution owner's guarded clock; this proves no worker/native terminal.
    pub(in crate::production) fn check_deadline(
        &self,
        now: Moment,
    ) -> Result<(), ReporterSourceIssue> {
        match &self.work {
            RelayWork::Admitted(_) | RelayWork::Rejected(_) => return Ok(()),
            RelayWork::Delivered(done)
                if !matches!(done.outcome, JobOutcome::Success(worker::RelayTurn::Deferred)) =>
            {
                return Ok(());
            }
            RelayWork::Idle | RelayWork::Delivered(_) => {}
        }
        let until = self.deadline().ok_or(ReporterSourceIssue::Identity)?;
        if now >= until {
            return Err(ReporterSourceIssue::Deadline);
        }
        Ok(())
    }
    /// Observe an escaped original panic while the loan has restored the complete funded source.
    pub(in crate::production) fn panic_cause(&self) -> Option<&PanicCause> {
        match &self.work {
            RelayWork::Delivered(JobCompletion {
                outcome: JobOutcome::Panicked(cause), ..
            }) => Some(cause),
            RelayWork::Idle
            | RelayWork::Admitted(_)
            | RelayWork::Delivered(_)
            | RelayWork::Rejected(_) => None,
        }
    }
    fn capacity_rejection(cause: &AdmissionError) -> bool {
        matches!(
            cause,
            AdmissionError::QueueSaturated { .. }
                | AdmissionError::CompletionSaturated
                | AdmissionError::MemorySaturated { .. }
        )
    }
    /// Retry only genuine capacity pressure; a terminal admission keeps its own exact cause.
    pub(in crate::production) fn admission_retryable(&self) -> bool {
        !matches!(self.work, RelayWork::Rejected(_))
            && self.admission.as_ref().is_some_and(Self::capacity_rejection)
    }
    /// Borrow central admission cause without replacing its original metadata.
    pub(in crate::production) fn admission_failure(&self) -> Option<&AdmissionError> {
        match &self.work {
            RelayWork::Rejected(cause) => Some(cause),
            RelayWork::Idle | RelayWork::Admitted(_) | RelayWork::Delivered(_) => {
                self.admission.as_ref()
            }
        }
    }
    /// Borrow the actual original finite worker's failure after its matched terminal delivery.
    pub(in crate::production) fn worker_failure(&self) -> Option<&ReporterSourceIssue> {
        match &self.work {
            RelayWork::Delivered(JobCompletion { outcome: JobOutcome::Failure(cause), .. }) => {
                Some(cause)
            }
            RelayWork::Idle
            | RelayWork::Admitted(_)
            | RelayWork::Delivered(_)
            | RelayWork::Rejected(_) => None,
        }
    }
    /// Return only the matched actual terminal, including cancellation/deadline without settlement.
    pub(in crate::production) fn worker_terminal(&self) -> Option<TerminalKind> {
        match &self.work {
            RelayWork::Delivered(done) if self.matched => Some(done.outcome.kind()),
            RelayWork::Idle
            | RelayWork::Admitted(_)
            | RelayWork::Delivered(_)
            | RelayWork::Rejected(_) => None,
        }
    }
    /// Actual delivered means only this finite worker has completed, never native-family settlement.
    pub(in crate::production) fn worker_delivered(&self) -> bool {
        !matches!(self.work, RelayWork::Admitted(_))
    }
    /// Request cancellation without dropping source or claiming native or worker settlement.
    pub(in crate::production) fn cancel(&self) {
        if let RelayWork::Admitted(job) = &self.work {
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
        if self.work.has_failure(self.matched) && failure.is_none() {
            self.source.loan()?.state()?.failure_transferred = true;
        }
        if let Some(retained) = self.work.take_failure(self.matched) {
            if let Some(failure) = &mut failure {
                failure._secondary = Some(retained);
            } else {
                let cause = match retained {
                    failure::ReporterSecondaryFailure::Worker { _completion: done } => {
                        OriginalCause::Worker(done)
                    }
                    failure::ReporterSecondaryFailure::Admission { _cause: cause } => {
                        OriginalCause::Admission(cause)
                    }
                };
                failure = Some(OwnedReporterFailure {
                    cause,
                    _secondary: None,
                    _admission: None,
                    _source: self.source.alias(),
                });
            }
        }
        if let Some(failure) = &mut failure {
            failure._admission = self.admission.take();
        }
        Ok(failure)
    }
}

#[cfg(test)]
#[path = "controller/held_loan_tests.rs"]
mod held_loan_tests;
