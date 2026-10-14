// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore only an original passive input, retaining real admission and terminal custody.
use super::*;

/// Constructor-private passive disposition; it grants no native or file retirement proof.
/// The original execution/scope/cutoff and any genuine unstarted terminal remain immutable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) struct PassiveReportingRestoration {
    original: (ExecutionId, ScopeId, Moment),
    terminal: Option<TerminalKind>,
    operation: LogicalStoreOperationKey,
    binding: ReportBinding,
    symbols: bool,
}
impl PassiveReportingRestoration {
    /// Read only the original service submission binding, without allocating replacement work.
    pub(in crate::production) const fn original(&self) -> (ExecutionId, ScopeId, Moment) {
        self.original
    }
    /// Read the actual delivered unstarted terminal; None means no Core job was admitted.
    pub(in crate::production) const fn terminal(&self) -> Option<TerminalKind> {
        self.terminal
    }
    /// Observe the actual original work kind; copied binding metadata cannot become Symbols.
    pub(in crate::production) const fn is_symbols(&self) -> bool {
        self.symbols
    }
    /// Read the logical operation from the same original returned pristine input.
    pub(in crate::production) const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Read the exact report binding from the same original returned pristine input.
    pub(in crate::production) const fn binding(&self) -> ReportBinding {
        self.binding
    }
}
impl ReportingService {
    /// Return the identical never-issued input outside locks and permanently close the source.
    /// Original admission/identity causes and original requested binding stay retained.
    /// # Errors
    /// Rejects an occupied caller slot, admitted job, delivery, presentation, cleanup or begun source.
    pub(in crate::production) fn restore_unsubmitted(
        &mut self,
        original: &mut Option<ReportingInput>,
    ) -> Result<PassiveReportingRestoration, ReportingIssue> {
        self.restore_original(original, false)
    }
    /// Restore after genuine Cancelled/TimedOut delivery only if Source.begin never ran.
    /// The same real completion and its original memory stay retained; no cleanup is synthesized.
    /// # Errors
    /// Rejects other outcomes, mismatched/active work and every post-effect source state.
    pub(in crate::production) fn restore_unstarted(
        &mut self,
        original: &mut Option<ReportingInput>,
    ) -> Result<PassiveReportingRestoration, ReportingIssue> {
        self.restore_original(original, true)
    }
    fn restore_original(
        &mut self,
        original: &mut Option<ReportingInput>,
        admitted: bool,
    ) -> Result<PassiveReportingRestoration, ReportingIssue> {
        if original.is_some()
            || self.restored.is_some()
            || !self.installed
            || self.work.job.is_some()
            || self.presentation.occupied()
            || self.cleanup.occupied()
            || self.view_slot.is_some()
            || self.view_binding.is_some()
        {
            return Err(ReportingIssue::Busy);
        }
        let binding = self.requested.ok_or(ReportingIssue::Identity)?;
        let terminal = match (&self.work.completion, admitted) {
            (None, false) if self.work.terminal.is_none() => None,
            (Some(done), true)
                if done.scope_id == binding.1
                    && self.work.terminal == Some(done.outcome.kind())
                    && matches!(
                        done.outcome.kind(),
                        TerminalKind::Cancelled | TerminalKind::TimedOut
                    )
                    && done.spec.deadline == Some(binding.2)
                    && done.spec.reservation_bytes == DIAGNOSTIC_STORAGE_BYTES =>
            {
                Some(done.outcome.kind())
            }
            _ => return Err(ReportingIssue::Busy),
        };
        let input = self.source.restore_passive()?;
        let symbols = matches!(&input.work, ReportingWork::Symbols { .. });
        let operation = input.operation;
        let input_binding = input.work.binding();
        *original = Some(input);
        let restored = PassiveReportingRestoration {
            original: binding,
            terminal,
            operation,
            binding: input_binding,
            symbols,
        };
        self.restored = Some(restored);
        Ok(restored)
    }
    /// Borrow the immutable passive marker after restoration, excluding file/worker retirement.
    pub(in crate::production) fn passive_restoration(&self) -> Option<PassiveReportingRestoration> {
        self.restored
    }
    /// Borrow the original first admission rejection, including after checked passive restoration.
    pub(in crate::production) fn admission_failure(&self) -> Option<&AdmissionError> {
        self.admission.as_ref()
    }
    /// Borrow the original first sequence rejection without replacing its cause or binding.
    pub(in crate::production) fn identity_failure(&self) -> Option<&IdentityError> {
        self.identity.as_ref()
    }
}
