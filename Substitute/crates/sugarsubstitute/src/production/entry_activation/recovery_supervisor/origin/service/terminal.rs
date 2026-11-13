// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate actual terminal identities and original reservations before outcome adoption.
use super::*;
fn notice<T, E>(
    job: &OwnedJob<T, E>,
    terminal: &mut Option<TerminalKind>,
    batch: &CompletionBatch,
) -> Result<(), OriginIssue> {
    for value in &batch.notifications {
        if (value.scope_id, value.job_id) != (job.scope_id(), job.spec().id) {
            continue;
        }
        if value.replacement.is_some() || terminal.is_some_and(|old| old != value.kind) {
            return Err(OriginIssue::Identity);
        }
        *terminal = Some(value.kind);
    }
    Ok(())
}
fn matches<T, E>(
    done: &JobCompletion<T, E>,
    scope: ScopeId,
    spec: JobSpec,
    kind: Option<TerminalKind>,
) -> bool {
    done.scope_id == scope
        && done.spec == spec
        && Some(done.outcome.kind()) == kind
        && done.memory.bytes() == spec.reservation_bytes
        && done.native_handles.is_none()
}
impl OriginPublication {
    pub(super) fn capture_work(&mut self, batch: &CompletionBatch) -> Result<(), OriginIssue> {
        let Some(job) = &mut self.job else {
            return Ok(());
        };
        if let Err(cause) = notice(job, &mut self.terminal, batch) {
            self.identity_rejected = true;
            return Err(cause);
        }
        if self.terminal.is_some()
            && let Some(done) = job.try_take_completion()
        {
            let valid = matches(&done, job.scope_id(), job.spec(), self.terminal)
                && self
                    .original
                    .is_some_and(|(_, scope, spec)| (scope, spec) == (done.scope_id, done.spec));
            self.completion = Some(RetainedOutcome { original: done });
            self.job = None;
            if !valid {
                self.identity_rejected = true;
                return Err(OriginIssue::Identity);
            }
        }
        Ok(())
    }
    pub(super) fn capture_cleanup(&mut self, batch: &CompletionBatch) -> Result<(), OriginIssue> {
        let Some(job) = &mut self.cleanup else {
            return Ok(());
        };
        if let Err(cause) = notice(job, &mut self.cleanup_terminal, batch) {
            self.identity_rejected = true;
            return Err(cause);
        }
        if self.cleanup_terminal.is_some()
            && let Some(done) = job.try_take_completion()
        {
            let valid = matches(&done, job.scope_id(), job.spec(), self.cleanup_terminal)
                && self
                    .cleanup_original
                    .is_some_and(|(_, scope, spec)| (scope, spec) == (done.scope_id, done.spec));
            self.cleanup_completion = Some(RetainedOutcome { original: done });
            self.cleanup = None;
            if !valid {
                self.identity_rejected = true;
                return Err(OriginIssue::Identity);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "terminal/tests.rs"]
mod tests;
