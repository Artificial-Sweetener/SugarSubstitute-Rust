// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist once-counted session outcomes and actual same-session view refresh.
use super::{
    super::{
        state::{MutationCommit, Pending},
        *,
    },
    rewritten,
};
use sugarsubstitute_contracts::incident::*;
impl IncidentCoordinator {
    /// Preflight one actual outcome without launching another reporter or concurrent CAS.
    pub(in crate::incident) fn record_attempt(
        &mut self,
        binding: ReportBinding,
        attempt: ReportAttempt,
        context: IncidentRequestContext,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(binding.incident())?;
        let record = self.record(index)?;
        if let Some(queued) = &record.queued_failure
            && let ReportAttempt::Failed(failure) = attempt
        {
            return if queued.observed_binding == binding
                && queued.failure == failure
                && queued.context == context
            {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::ChangedDuplicate)
            };
        }
        if record.last_attempt == Some((attempt, context))
            && (record.binding == Some(binding)
                || record.last_commit.as_ref().is_some_and(|commit| commit.binding == binding))
        {
            return Ok(IncidentEffects::default());
        }
        if record.binding != Some(binding)
            || attempt == ReportAttempt::Presented
                && (record.reporter_retired || record.acknowledgement.is_some())
        {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if (record.pending.is_some() || record.refresh_pending)
            && let ReportAttempt::Failed(failure) = attempt
        {
            return self.queue_report_failure(index, binding, failure, context);
        }
        if record.pending.is_some() || record.refresh_pending {
            return Err(IncidentDecisionError::WrongPhase);
        }
        if attempt == ReportAttempt::Presented && record.attempted_session {
            return Err(IncidentDecisionError::ChangedDuplicate);
        }
        let summary = record.summary.as_ref().ok_or(IncidentDecisionError::WrongPhase)?;
        let attempts = summary.facts().reporting().attempts();
        let attempts = if record.attempted_session {
            attempts
        } else {
            attempts.checked_add(1).ok_or(IncidentDecisionError::OperationExhausted)?
        };
        let expected = rewritten(
            record,
            ReportDisposition::new(attempts, Some(attempt))?,
            summary.facts().retention().acknowledged(),
        )?;
        let scope = self.scope(binding.incident(), summary.revision(), context.budget())?;
        let effects = IncidentEffects::one(IncidentEffect::RecordAttempt {
            scope,
            binding,
            attempt,
            at: context.at(),
        });
        let record = self.record_mut(index)?;
        record.pending = Some(Pending::Attempt { binding, expected, attempt, at: context.at() });
        record.current = Some(scope);
        record.last_attempt = Some((attempt, context));
        record.presented = attempt == ReportAttempt::Presented;
        self.next_operation = scope.operation().id().get();
        Ok(effects)
    }
    /// Accept exact actual disposition and refresh only an accessible live report view.
    pub(in crate::incident) fn attempt_committed(
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
        let Some(Pending::Attempt { binding: original, expected, attempt, .. }) = &record.pending
        else {
            return Err(IncidentDecisionError::WrongPhase);
        };
        if *original != binding || *expected != summary {
            return Err(IncidentDecisionError::MismatchedCommit);
        }
        let current = ReportBinding::new(binding.incident(), summary.revision(), binding.session());
        let presented = *attempt == ReportAttempt::Presented;
        let refresh = !record.reporter_retired && record.acknowledgement.is_none();
        let effects = if refresh {
            IncidentEffects::one(IncidentEffect::ReplaceReportView {
                old: binding,
                current,
                summary: summary.clone(),
            })
        } else {
            IncidentEffects::default()
        };
        let record = self.record_mut(index)?;
        record.summary = Some(summary);
        record.binding = Some(current);
        record.pending = None;
        record.current = None;
        record.last_commit = Some(commit);
        record.attempted_session = true;
        record.refresh_pending = refresh;
        record.presented = presented && record.queued_failure.is_none() && refresh;
        if !refresh && let Some(effect) = self.activate_queued_attempt(index)? {
            return Ok(IncidentEffects::one(effect));
        }
        Ok(effects)
    }
    /// Unfence a live view only after actual consumption, then emit the preflighted queued CAS.
    pub(in crate::incident) fn view_replaced(
        &mut self,
        binding: ReportBinding,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(binding.incident())?;
        let record = self.record(index)?;
        if record.binding != Some(binding) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if !record.refresh_pending {
            return Ok(IncidentEffects::default());
        }
        self.record_mut(index)?.refresh_pending = false;
        if let Some(effect) = self.activate_queued_attempt(index)? {
            return Ok(IncidentEffects::one(effect));
        }
        Ok(IncidentEffects::default())
    }
}
