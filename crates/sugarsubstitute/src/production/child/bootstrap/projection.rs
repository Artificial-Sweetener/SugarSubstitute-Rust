// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match configuration notices to complete original guarded results before transfer.
use super::{ChildBootstrap, ChildBootstrapIssue, CompletionBatch, JobOutcome};

impl ChildBootstrap {
    /// Retain failed or late results and publish only the original successful projection.
    pub(super) fn capture_projection(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(), ChildBootstrapIssue> {
        let Some(job) = &mut self.projection else { return Ok(()) };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                if self.notice.is_some_and(|kind| kind != notice.kind) {
                    return Err(ChildBootstrapIssue::Identity);
                }
                self.notice = Some(notice.kind);
            }
        }
        if self.notice.is_none() {
            return Ok(());
        }
        let Some(completion) = job.try_take_completion() else { return Ok(()) };
        let matches = completion.scope_id == job.scope_id()
            && completion.spec == job.spec()
            && Some(completion.outcome.kind()) == self.notice;
        self.projection = None;
        self.notice = None;
        if !matches || self.closing || !matches!(completion.outcome, JobOutcome::Success(_)) {
            self.failed_projection = Some(completion);
            let issue = if matches {
                ChildBootstrapIssue::Configuration
            } else {
                ChildBootstrapIssue::Identity
            };
            self.issue.get_or_insert(issue);
            self.close_initialization();
            return Err(issue);
        }
        let JobOutcome::Success(configuration) = completion.outcome else {
            return Err(ChildBootstrapIssue::Configuration);
        };
        self.prepared = Some(configuration.into_startup(self.identity));
        Ok(())
    }
}
