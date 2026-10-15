// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt the real original service terminal before returning the same source policy.
use super::*;
impl CollectionService {
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
        policy: &mut Option<FundedState>,
    ) -> Result<bool, CollectionIssue> {
        let (matched, adopted) = self.capture_terminal(batch)?;
        if adopted {
            self.restore_policy(policy)?;
        }
        Ok(matched)
    }
    /// Adopt only the actual matching original completion before restoring its policy.
    pub(super) fn capture_terminal(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(bool, bool), CollectionIssue> {
        let Some(job) = &mut self.job else { return Ok((false, false)) };
        let mut matched = false;
        let mut adopted = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|old| old != notice.kind) {
                return Err(CollectionIssue::Identity);
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
                return Err(CollectionIssue::Identity);
            }
            adopted = true;
        }
        Ok((matched, adopted))
    }
    /// Restore the original source value after the retained terminal passed every identity check.
    pub(super) fn restore_policy(
        &mut self,
        policy: &mut Option<FundedState>,
    ) -> Result<(), CollectionIssue> {
        self.passive_restored = self.source.restore(policy)?;
        if self.passive_restored.is_some() {
            self.passive_observation_pending = true;
            self.installed = false;
        }
        Ok(())
    }
}
