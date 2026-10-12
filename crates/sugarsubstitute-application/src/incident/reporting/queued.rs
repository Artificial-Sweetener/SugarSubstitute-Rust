// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight one subsequent failure before changing IDs or the preceding mutation.
use super::super::{
    state::{Pending, QueuedAttempt},
    *,
};
use sugarsubstitute_contracts::incident::*;

impl IncidentCoordinator {
    /// Reserve exact failure content after the already-prepared successor, without a concurrent CAS.
    pub(in crate::incident) fn queue_report_failure(
        &mut self,
        index: usize,
        binding: ReportBinding,
        failure: ReportFailure,
        context: IncidentRequestContext,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let record = self.record(index)?;
        if let Some(queued) = &record.queued_failure {
            return if queued.observed_binding == binding
                && queued.failure == failure
                && queued.context == context
            {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::ChangedDuplicate)
            };
        }
        let (summary, target_binding) = match record.pending.as_ref() {
            Some(Pending::Attempt { expected, .. }) => (
                expected,
                ReportBinding::new(binding.incident(), expected.revision(), binding.session()),
            ),
            Some(Pending::Acknowledgement { expected, .. }) => (expected, binding),
            Some(Pending::Export(_)) | None => {
                (record.summary.as_ref().ok_or(IncidentDecisionError::WrongPhase)?, binding)
            }
            _ => return Err(IncidentDecisionError::WrongPhase),
        };
        let facts = summary.facts();
        let attempt = ReportAttempt::Failed(failure);
        let expected = IncidentSummary::new(
            summary.seed().clone(),
            summary.revision().next()?,
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                ReportDisposition::new(facts.reporting().attempts(), Some(attempt))?,
                facts.symbols().clone(),
            ),
        )?;
        let scope = self.scope(binding.incident(), summary.revision(), context.budget())?;
        let queued = QueuedAttempt {
            failure,
            context,
            observed_binding: binding,
            binding: target_binding,
            scope,
            expected,
        };
        let record = self.record_mut(index)?;
        record.queued_failure = Some(queued);
        record.presented = false;
        self.next_operation = scope.operation().id().get();
        Ok(IncidentEffects::default())
    }
    /// Move the already-prepared queued CAS into its one current mutation slot.
    pub(in crate::incident) fn activate_queued_attempt(
        &mut self,
        index: usize,
    ) -> Result<Option<IncidentEffect>, IncidentDecisionError> {
        let record = self.record_mut(index)?;
        let Some(queued) = record.queued_failure.take() else {
            return Ok(None);
        };
        let attempt = ReportAttempt::Failed(queued.failure);
        let effect = IncidentEffect::RecordAttempt {
            scope: queued.scope,
            binding: queued.binding,
            attempt,
            at: queued.context.at(),
        };
        record.current = Some(queued.scope);
        record.pending = Some(Pending::Attempt {
            binding: queued.binding,
            expected: queued.expected,
            attempt,
            at: queued.context.at(),
        });
        record.last_attempt = Some((attempt, queued.context));
        Ok(Some(effect))
    }
}
