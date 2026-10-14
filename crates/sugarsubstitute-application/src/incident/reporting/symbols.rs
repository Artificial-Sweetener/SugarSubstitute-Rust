// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize actual symbol metadata before the first native report presentation.
use super::*;
impl IncidentCoordinator {
    /// Admit one exact symbol metadata mutation after the original launch effect is retained.
    /// The caller keeps its original interval through reading, symbol lookup and metadata CAS.
    /// This scope grants no native or storage authority and does not create another session.
    /// # Errors
    /// Rejects stale, presented, retired, already pending or dump-unavailable records.
    pub fn admit_report_symbols(
        &mut self,
        binding: ReportBinding,
        context: IncidentRequestContext,
    ) -> Result<IncidentOperationScope, IncidentDecisionError> {
        let index = self.index(binding.incident())?;
        let record = self.record(index)?;
        if record.binding != Some(binding) || record.revision() != binding.revision() {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let summary = record.summary.as_ref().ok_or(IncidentDecisionError::WrongPhase)?;
        if record.presented
            || record.reporter_retired
            || record.pending.is_some()
            || !matches!(summary.facts().capture(), CaptureEvidence::Finished(_))
            || matches!(summary.facts().symbols(), SymbolOutcome::Resolved(_))
        {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let scope = self.scope(binding.incident(), binding.revision(), context.budget())?;
        let original = summary.clone();
        self.next_operation = scope.operation().id().get();
        let record = self.record_mut(index)?;
        record.current = Some(scope);
        record.pending = Some(Pending::Symbols { binding, original });
        Ok(scope)
    }
    /// Accept only the exact next revision and symbol-only change from the composing proof owner.
    pub(in crate::incident) fn symbols_committed(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
        summary: IncidentSummary,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        let commit = MutationCommit { scope, binding, summary: summary.clone() };
        if record.last_commit.as_ref() == Some(&commit) {
            return Ok(IncidentEffects::default());
        }
        if record.current != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let Some(Pending::Symbols { binding: expected, original }) = &record.pending else {
            return Err(IncidentDecisionError::WrongPhase);
        };
        let before = original.facts();
        let after = summary.facts();
        let frame_matches = match (after.symbols(), before.capture()) {
            (SymbolOutcome::Resolved(frame), CaptureEvidence::Finished(dump)) => {
                frame.dump() == dump && frame.release().build() == original.seed().build()
            }
            _ => false,
        };
        if *expected != binding
            || summary.seed() != original.seed()
            || original.revision().next()? != summary.revision()
            || before.retention() != after.retention()
            || before.exit() != after.exit()
            || before.failure() != after.failure()
            || before.capture() != after.capture()
            || before.payload() != after.payload()
            || before.reporting() != after.reporting()
            || !frame_matches
        {
            return Err(IncidentDecisionError::MismatchedCommit);
        }
        let current = ReportBinding::new(binding.incident(), summary.revision(), binding.session());
        let effects = IncidentEffects::one(IncidentEffect::LaunchReporter {
            binding: current,
            summary: summary.clone(),
        });
        let record = self.record_mut(index)?;
        record.summary = Some(summary);
        record.binding = Some(current);
        record.current = None;
        record.pending = None;
        record.last_commit = Some(commit);
        record.phase = IncidentServicePhase::Reporting;
        Ok(effects)
    }
}

impl IncidentCoordinator {
    /// Restore reporting only after the host proves this exact mutation never began.
    /// This projection grants no source reuse, file retirement or publication authority.
    pub(in crate::incident) fn symbols_not_written(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.current != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let Some(Pending::Symbols { binding: expected, original }) = &record.pending else {
            return Err(IncidentDecisionError::WrongPhase);
        };
        if *expected != binding
            || record.binding != Some(binding)
            || record.summary.as_ref() != Some(original)
            || binding.incident() != scope.incident()
            || binding.revision() != scope.expected_revision()
            || record.presented
            || record.reporter_retired
        {
            return Err(IncidentDecisionError::MismatchedCommit);
        }
        let effects = IncidentEffects::one(IncidentEffect::LaunchReporter {
            binding,
            summary: original.clone(),
        });
        let record = self.record_mut(index)?;
        record.current = None;
        record.pending = None;
        record.phase = IncidentServicePhase::Reporting;
        Ok(effects)
    }
}
