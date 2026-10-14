// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare immutable real stored facts on bounded presentation credit at the original cutoff.
use super::*;
impl ReportingService {
    /// Submit a separate finite320KiB job while retaining the genuine original Core source.
    /// Two nonclone slots bound current and replacement owners through native disposition.
    pub(in crate::production) fn submit_view(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), ReportingIssue> {
        if !self.work.succeeded()
            || self.presentation.occupied()
            || self.cleanup.job.is_some()
            || self.source.inspect(ReportingOperation::failed) != Some(false)
            || self.source.inspect(|operation| operation.retiring_authentication) != Some(false)
        {
            return Err(ReportingIssue::Busy);
        }
        if self.requested.is_none_or(|old| old.0 != scope.owner_id() || until > old.2)
            || self.view_binding.is_some_and(|old| old != (scope.id(), until))
            || self.source.inspect(|operation| operation.view_request.is_some()) != Some(true)
            || self.source.inspect(|operation| operation.original_deadline) != Some(Some(until))
        {
            return Err(ReportingIssue::Identity);
        }
        self.view_binding = Some((scope.id(), until));
        if self.view_slot.is_none() {
            self.view_slot = Some(self.source.reserve_view()?);
        }
        let id = self.allocate(jobs)?;
        let source = self.source.alias();
        let mut spec = Self::spec(id, until, REPORT_VIEW_JOB_BYTES);
        spec.operation = Operation::Infrastructure;
        match client.submit_blocking(scope, spec, move |context| {
            let mut loan = source.loan()?;
            let operation = loan.operation()?;
            if let Err(cause) = operation.prepare_view(&context) {
                operation.first.get_or_insert(cause);
            }
            Ok(!operation.failed() && !operation.retiring_authentication)
        }) {
            Ok(job) => {
                self.presentation.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(ReportingIssue::Admission)
            }
        }
    }
    /// Capture actual central delivery without implying streaming or native view disposal.
    pub(in crate::production) fn capture_view(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, ReportingIssue> {
        self.presentation.capture(batch)
    }
    /// Observe the actual delivered presentation terminal without claiming formatting success.
    pub(in crate::production) fn view_outcome(&self) -> Option<TerminalKind> {
        self.presentation.completion.as_ref().map(|done| done.outcome.kind())
    }
    /// Return the genuine Core's effective first cutoff, including any tighter ancestor scope.
    pub(in crate::production) fn view_cutoff(&self) -> Option<Moment> {
        self.source.inspect(|operation| operation.original_deadline).flatten()
    }
}
