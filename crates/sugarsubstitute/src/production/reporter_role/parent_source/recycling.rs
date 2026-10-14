// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold the same source allocation closed through checked retirement and accepted disposition.
use super::*;
/// Exact original native retirement and same source allocation, retained across pending adoption.
pub(in crate::production) struct RecycledReporter {
    parent: Box<ReporterParent>,
    source: ReporterSource,
}
/// Nonclone passive disposition issued only after the same genuine closed source resets.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::production) struct DisposedReporter {
    key: DiagnosticKey,
}
impl DisposedReporter {
    /// Exact disposed session correlation; this grants no new native effect or deadline.
    pub(in crate::production) const fn key(&self) -> DiagnosticKey {
        self.key
    }
}
impl RecycledReporter {
    /// Borrow unchanged original session correlation; this grants no native effect.
    pub(in crate::production) fn key(&self) -> DiagnosticKey {
        self.parent.key()
    }
    /// Borrow the actual registered process whose same family was checked before detach.
    pub(in crate::production) fn process(&self) -> Option<ProcessKey> {
        self.parent.process()
    }
    /// Preserve the actual deliberate cancellation independently of checked physical retirement.
    pub(in crate::production) fn native_outcome(&self) -> Option<TerminalKind> {
        self.parent.recycled_outcome()
    }
}
impl ReporterSource {
    /// Compare actual allocation identity without granting native effects or retirement.
    pub(in crate::production) fn same_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
    /// Observe a genuinely empty funded allocation after accepted old-session disposal.
    pub(super) fn idle_funded(&self) -> bool {
        let Ok(mut loan) = self.loan() else { return false };
        let Ok(state) = loan.state() else { return false };
        state.parent.is_none()
            && state.funding.is_some()
            && !state.closing
            && state.views.iter().all(Option::is_none)
            && state.completion.is_none()
            && state.control.is_none()
            && state.control_write.is_none()
            && state.output.is_none()
            && state.rejected.is_none()
            && state.first.is_none()
            && state.wire.is_none()
            && state.time_cause.is_none()
            && state.registry_cause.is_none()
            && !state.failure_transferred
    }
    fn healthy_closing(state: &State) -> Result<(), ReporterSourceIssue> {
        if !state.closing {
            return Err(ReporterSourceIssue::Identity);
        }
        if state.first.is_some()
            || state.wire.is_some()
            || state.time_cause.is_some()
            || state.registry_cause.is_some()
            || state.failure_transferred
        {
            return Err(ReporterSourceIssue::Parent);
        }
        Ok(())
    }
    /// Borrow-validation before releasing any matched healthy launch alias.
    pub(super) fn recycling_ready(
        &self,
        registry: &WindowsProcessRegistry,
        expected: DiagnosticKey,
    ) -> Result<bool, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        Self::healthy_closing(state)?;
        if state.funding.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        state
            .parent
            .as_deref_mut()
            .ok_or(ReporterSourceIssue::Busy)?
            .recycling_ready(registry, expected)
            .map_err(|_| ReporterSourceIssue::Parent)
    }
    fn recycling_loan(&mut self) -> Result<Loan<'_>, ReporterSourceIssue> {
        if Arc::strong_count(&self.shared) != 1 {
            return Err(ReporterSourceIssue::Busy);
        }
        let mut loan = self.loan()?;
        Self::healthy_closing(loan.state()?)?;
        Ok(loan)
    }
    /// Detach only after every external preparation alias returned and the actual family is settled.
    pub(super) fn detach_closed(
        &mut self,
        registry: &WindowsProcessRegistry,
        expected: DiagnosticKey,
    ) -> Result<Option<RecycledReporter>, ReporterSourceIssue> {
        let mut loan = self.recycling_loan()?;
        let state = loan.state()?;
        if state.funding.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        let parent = state.parent.as_deref_mut().ok_or(ReporterSourceIssue::Busy)?;
        if !parent.recycling_ready(registry, expected).map_err(|_| ReporterSourceIssue::Parent)? {
            return Ok(None);
        }
        // Move only the same Box handle; its original terminal, proof and claim stay together.
        let parent = state.parent.take().ok_or(ReporterSourceIssue::Busy)?;
        drop(loan);
        Ok(Some(RecycledReporter { parent, source: self.alias() }))
    }
    /// Consume original receipt only at the receiving owner's accepted retirement/ingress boundary.
    /// Denial leaves receipt, same funded source and all old session metadata unchanged.
    pub(super) fn finish_recycled(
        &mut self,
        receipt: &mut Option<RecycledReporter>,
    ) -> Result<DisposedReporter, ReporterSourceIssue> {
        let original = receipt.as_ref().ok_or(ReporterSourceIssue::Busy)?;
        if !self.same_allocation(&original.source)
            || !original.parent.recycled_owner_matches(self.shared.owner)
        {
            return Err(ReporterSourceIssue::Identity);
        }
        if Arc::strong_count(&self.shared) != 2 {
            return Err(ReporterSourceIssue::Busy);
        }
        let mut loan = self.loan()?;
        let state = loan.state()?;
        Self::healthy_closing(state)?;
        if state.parent.is_some() || state.funding.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        let key = original.parent.key();
        let original = receipt.as_mut().ok_or(ReporterSourceIssue::Busy)?;
        // No fallible work follows the transfer of the same original claim.
        let funding = original
            .parent
            .release_recycled_funding(&self.outside)
            .map_err(|_| ReporterSourceIssue::Parent)?;
        state.funding = Some(funding);
        // The same Parent Box retains proof and terminal until this explicit accepted boundary.
        drop(receipt.take());
        state.views = [None, None];
        state.completion = None;
        state.control = None;
        state.control_write = None;
        state.control_written = false;
        state.output = None;
        state.rejected = None;
        state.read_after_complete = false;
        state.core_scope = None;
        state.last_observed = None;
        state.closing = false;
        Ok(DisposedReporter { key })
    }
}
#[cfg(test)]
#[path = "recycling/tests.rs"]
mod tests;
