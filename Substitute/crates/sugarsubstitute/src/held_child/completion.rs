// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acknowledge only exact typed child terminals; cancellation never proves retirement.
use super::{
    Result,
    failure::{ChildFailure, terminal},
};
use std::error::Error;
use sugarsubstitute_contracts::execution::{CompletionBatch, Moment, OwnedJob, TerminalKind};
use sugarsubstitute_execution::{
    windows::{NativeCorrelation, NativeObservationError},
    windows_fixture::FixtureChildReport,
};

type ChildJob = OwnedJob<FixtureChildReport, NativeObservationError>;
pub(super) struct ChildCompletion {
    correlation: NativeCorrelation,
    job: Option<ChildJob>,
    deadline: Option<Moment>,
    notice: Option<TerminalKind>,
    report: Option<FixtureChildReport>,
    terminal_consumed: bool,
    closing: bool,
    deadline_failure_observed: bool,
}
impl ChildCompletion {
    pub(super) const fn new(correlation: NativeCorrelation) -> Self {
        Self {
            correlation,
            job: None,
            deadline: None,
            notice: None,
            report: None,
            terminal_consumed: false,
            closing: false,
            deadline_failure_observed: false,
        }
    }
    pub(super) fn install(&mut self, job: ChildJob) -> Result<()> {
        if self.job.is_some() || self.terminal_consumed || self.closing {
            return Err("Held child already owns or closed its admitted job".into());
        }
        self.deadline = job.spec().deadline;
        self.job = Some(job);
        if self.deadline.is_none() {
            return Err("Held child admission did not retain its derived deadline".into());
        }
        Ok(())
    }
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<()> {
        if batch.notifications.len() > 32 {
            return Err("Held child completion batch exceeds32".into());
        }
        let Some(job) = &mut self.job else { return Ok(()) };
        let expected = (job.scope_id(), job.spec());
        for notification in &batch.notifications {
            if (notification.scope_id, notification.job_id) == (expected.0, expected.1.id) {
                self.notice = Some(notification.kind);
            }
        }
        let Some(kind) = self.notice else { return Ok(()) };
        let Some(completion) = job.try_take_completion() else { return Ok(()) };
        self.job = None;
        self.notice = None;
        self.terminal_consumed = true;
        if completion.scope_id != expected.0
            || completion.spec != expected.1
            || completion.outcome.kind() != kind
        {
            return Err(Box::new(ChildFailure::<NativeObservationError>::Protocol));
        }
        if let sugarsubstitute_contracts::execution::JobOutcome::Failure(error) =
            &completion.outcome
        {
            tracing::error!(operation = "held_child_native_terminal", correlation = self.correlation.get(),
                stage = error.stage(), owner_panic = error.contains_owner_panic(), native_cause = ?error.source(),
                "Held child retains an original native failure");
        }
        self.report = Some(terminal(self.correlation, completion.outcome)?);
        Ok(())
    }
    pub(super) fn request_close(&mut self) -> bool {
        if self.closing {
            return false;
        }
        self.closing = true;
        if let Some(job) = &self.job {
            job.cancel()
        }
        true
    }
    pub(super) const fn closing(&self) -> bool {
        self.closing
    }
    pub(super) const fn terminal_consumed(&self) -> bool {
        self.terminal_consumed
    }
    pub(super) const fn report(&self) -> Option<FixtureChildReport> {
        self.report
    }
    pub(super) fn shutdown_deadline(&self, unbound_now: Moment) -> Moment {
        self.deadline.unwrap_or(unbound_now)
    }
    pub(super) fn shutdown_settled(&self, actual_workers_settled: bool) -> bool {
        self.closing && self.job.is_none() && actual_workers_settled
    }
    pub(super) fn observe_shutdown(
        &mut self,
        actual_workers_settled: bool,
        expired: bool,
    ) -> Result<bool> {
        if expired && !self.deadline_failure_observed {
            self.deadline_failure_observed = true;
            return Err(
                "Held child execution remains unresolved after the original admitted deadline"
                    .into(),
            );
        }
        Ok(self.shutdown_settled(actual_workers_settled))
    }
}
