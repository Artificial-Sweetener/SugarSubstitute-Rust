// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the original claim only after genuine family and same-allocation pipe closure.
use super::*;
impl ReporterParent {
    /// Validate actual old family before its registry entry is removed. Reject causes unchanged.
    pub(in crate::production::reporter_role) fn recycling_ready(
        &mut self,
        registry: &WindowsProcessRegistry,
        expected: DiagnosticKey,
    ) -> Result<bool, ReporterIssue> {
        if self.key() != expected || expected.role() != DiagnosticRole::Reporter {
            return Err(ReporterIssue::Role);
        }
        if self.failed() || self.retired_terminal.is_some() || self._claim.is_none() {
            return Err(ReporterIssue::Transport);
        }
        let Some(process) = self.process() else { return Ok(false) };
        let actual = registry.snapshot(process).map_err(|cause| {
            self.launch_error.get_or_insert(cause);
            ReporterIssue::Launch
        })?;
        if actual.binding != sugarsubstitute_supervision::FamilyBinding::Diagnostic(expected) {
            return Err(ReporterIssue::Role);
        }
        if actual.failed {
            return Err(ReporterIssue::Launch);
        }
        if !actual.settled || !self.pipe_settled() {
            return Ok(false);
        }
        let Some(terminal) = &self.channel.terminal else { return Ok(false) };
        if !matches!(terminal.outcome, JobOutcome::Success(_) | JobOutcome::Cancelled) {
            return Err(ReporterIssue::Transport);
        }
        if self.bootstrap.is_some()
            || self.enrollment.is_some()
            || self.input.is_some()
            || self.rejected_output.is_some()
            || !self.channel.closed_inputs_returned()
        {
            return Ok(false);
        }
        Ok(true)
    }
    /// Borrow the actual original terminal without changing its cancellation disposition.
    pub(in crate::production::reporter_role) fn recycled_outcome(&self) -> Option<TerminalKind> {
        self.channel.terminal.as_ref().map(|terminal| terminal.outcome.kind())
    }
    /// Recheck unchanged original closure custody after checked registry removal.
    pub(in crate::production::reporter_role) fn recycled_owner_matches(
        &self,
        owner: ExecutionId,
    ) -> bool {
        self.execution == owner
            && self._claim.is_some()
            && !self.failed()
            && self.retired_terminal.is_none()
            && self.pipe_settled()
            && matches!(
                self.recycled_outcome(),
                Some(TerminalKind::Success | TerminalKind::Cancelled)
            )
            && self.bootstrap.is_none()
            && self.enrollment.is_none()
            && self.input.is_none()
            && self.rejected_output.is_none()
            && self.channel.closed_inputs_returned()
    }
    /// Return the same original claim only at the accepted receiving disposal boundary.
    /// Whole native completion and checked proof stay in this same Parent allocation.
    pub(in crate::production::reporter_role) fn release_recycled_funding(
        &mut self,
        outside: &MemoryLease,
    ) -> Result<ReporterFunding, ReporterIssue> {
        if !self.recycled_owner_matches(self.execution) {
            return Err(ReporterIssue::Transport);
        }
        let claim = self._claim.take().ok_or(ReporterIssue::Busy)?;
        Ok(ReporterFunding { claim, original: outside.clone(), owner: self.execution })
    }
}
