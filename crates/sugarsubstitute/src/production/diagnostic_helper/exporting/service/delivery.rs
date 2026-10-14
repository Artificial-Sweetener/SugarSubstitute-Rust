// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match genuine central notifications to the same retained typed result and charge.
use super::*;
/// One exact original completion stays funded until final result adoption.
#[derive(Default)]
pub(super) struct Delivery {
    pub(super) job: Option<OwnedJob<bool, ExportIssue>>,
    pub(super) completion: Option<JobCompletion<bool, ExportIssue>>,
    pub(super) terminal: Option<TerminalKind>,
}
impl Delivery {
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<bool, ExportIssue> {
        let Some(job) = self.job.as_mut() else {
            return Ok(false);
        };
        let mut changed = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|kind| kind != notice.kind) {
                return Err(ExportIssue::Identity);
            }
            self.terminal = Some(notice.kind);
            changed = true;
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
                return Err(ExportIssue::Identity);
            }
        }
        Ok(changed)
    }
    pub(super) fn succeeded(&self) -> bool {
        matches!(
            self.completion.as_ref().map(|value| &value.outcome),
            Some(JobOutcome::Success(true))
        )
    }
}

impl Delivery {
    pub(super) fn unexpected(&self) -> bool {
        self.completion.as_ref().is_some_and(|done| !matches!(done.outcome, JobOutcome::Success(_)))
    }
}
impl std::fmt::Debug for Delivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportDelivery").field("terminal", &self.terminal).finish_non_exhaustive()
    }
}
impl std::fmt::Display for Delivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Original export worker stopped: {:?}", self.terminal)
    }
}
impl std::error::Error for Delivery {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Failure(cause)) => Some(cause),
            _ => None,
        }
    }
}
