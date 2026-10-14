// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit actual close-only work after the original seed terminal, without renewing its cutoff.
use super::*;
impl SeedService {
    /// Close the retained repository on one real32KiB finite cleanup job.
    /// Existing native/data causes remain funded by their original8MiB source;
    /// the new job funds central cleanup metadata only. The first cutoff never extends.
    pub(in crate::production) fn submit_cleanup(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), SeedServiceIssue> {
        if self.job.is_some()
            || !self.installed
            || self.cleanup.is_some()
            || self.cleanup_completion.is_some()
        {
            return Err(SeedServiceIssue::Busy);
        }
        if self.requested.is_none_or(|original| original.0 != scope.owner_id())
            || self.cleanup_binding.is_some_and(|old| old != (scope.id(), until))
        {
            return Err(SeedServiceIssue::Identity);
        }
        self.cleanup_binding = Some((scope.id(), until));
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
            reservation_bytes: 32 * 1024,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            let mut loan = source.loan()?;
            Ok(loan.operation()?.retire(&context))
        }) {
            Ok(job) => {
                self.cleanup = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(SeedServiceIssue::Admission)
            }
        }
    }
    /// Observe whether a real cleanup admission or terminal already owns retirement.
    pub(in crate::production) fn cleanup_started(&self) -> bool {
        self.cleanup.is_some() || self.cleanup_completion.is_some()
    }
    /// Consume only matching genuine cleanup terminal delivery, independently of seed result.
    pub(in crate::production) fn capture_cleanup(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, SeedServiceIssue> {
        let Some(job) = &mut self.cleanup else {
            return Ok(false);
        };
        let mut matched = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.cleanup_terminal.is_some_and(|old| old != notice.kind) {
                return Err(SeedServiceIssue::Identity);
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
                return Err(SeedServiceIssue::Identity);
            }
        }
        Ok(matched)
    }
    /// Observe checked file closure and its actual cleanup terminal; no process join is implied.
    pub(in crate::production) fn files_retired(&self) -> bool {
        matches!(
            self.cleanup_completion.as_ref().map(|value| &value.outcome),
            Some(JobOutcome::Success(true))
        ) && self.source.inspect(SeedOperation::files_retired) == Some(true)
    }
}
