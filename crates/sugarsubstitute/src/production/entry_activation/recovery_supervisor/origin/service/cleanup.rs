// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use one original fifteen-second cleanup transaction and actual32KiB native work.
use super::*;
impl OriginPublication {
    pub(super) fn advance_cleanup(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), OriginIssue> {
        if self.cleanup.is_some() || self.cleanup_completion.is_some() {
            return Ok(());
        }
        if scope.owner_id() != self.source.owner()? {
            return Err(OriginIssue::Identity);
        }
        if self.cleanup_clock_failed || self.cleanup_time.is_some() {
            return Err(OriginIssue::Cleanup);
        }
        if self.cleanup_budget.is_none() {
            let now = client.now().map_err(|cause| {
                self.cleanup_clock_failed = true;
                self.cleanup_admission.get_or_insert(AdmissionError::Unexpected(cause));
                OriginIssue::Admission
            })?;
            self.cleanup_budget = Some(ShutdownBudget::new(now).map_err(|cause| {
                self.cleanup_time = Some(cause);
                OriginIssue::Cleanup
            })?);
        }
        let budget = self.cleanup_budget.ok_or(OriginIssue::Cleanup)?;
        if self.cleanup_original.is_some_and(|(owner, id, spec)| {
            owner != scope.owner_id()
                || id != scope.id()
                || spec.deadline != Some(budget.settle_until)
        }) {
            return Err(OriginIssue::Identity);
        }
        let id = jobs.allocate().map_err(|cause| {
            self.identity.get_or_insert(cause);
            OriginIssue::Identity
        })?;
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(budget.settle_until),
            reservation_bytes: 32 * 1024,
            replacement: None,
        };
        self.cleanup_original = Some((scope.owner_id(), scope.id(), spec));
        let source = self.source.alias()?;
        match client
            .submit_blocking(scope, spec, move |context| source.loan(false)?.retire(&context))
        {
            Ok(job) => {
                self.cleanup = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.cleanup_admission.get_or_insert(cause);
                Err(OriginIssue::Admission)
            }
        }
    }
    pub(super) fn cleanup_succeeded(&self) -> bool {
        !self.identity_rejected
            && matches!(
                self.cleanup_completion.as_ref().map(|done| &done.original.outcome),
                Some(JobOutcome::Success(true))
            )
            && self.source.retired()
    }
    /// Fence publication and cancel its actual existing job, then observe checked cleanup.
    /// No caller boolean or cancellation request can manufacture native settlement.
    pub(in crate::production) fn close(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<bool, OriginIssue> {
        self.closing = true;
        if self.released {
            return Ok(true);
        }
        if let Some(job) = &self.job {
            job.cancel();
            return Ok(false);
        }
        self.advance_cleanup(client, scope, jobs)?;
        if self.identity_rejected {
            return Err(OriginIssue::Identity);
        }
        if self.cleanup_succeeded() {
            return Ok(true);
        }
        if self.cleanup_completion.is_some() {
            return Err(OriginIssue::Cleanup);
        }
        Ok(false)
    }
}
