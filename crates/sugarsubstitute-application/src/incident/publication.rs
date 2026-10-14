// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Accept exact actual publication content while preserving original failure attribution.
use super::{state::Pending, *};
use sugarsubstitute_contracts::incident::*;

impl IncidentCoordinator {
    /// Accept actual exact seed publication without treating it as a failed exit.
    pub(super) fn seed_committed(
        &mut self,
        scope: IncidentOperationScope,
        seed: IncidentSeed,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.seed_scope != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if record.seed != seed {
            return Err(IncidentDecisionError::MismatchedCommit);
        }
        if record.seed_committed {
            return Ok(IncidentEffects::default());
        }
        if record.pending != Some(Pending::Seed) || record.current != Some(scope) {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let record = self.record_mut(index)?;
        record.seed_committed = true;
        record.pending = None;
        record.current = None;
        record.phase = IncidentServicePhase::SeedCommitted;
        Ok(IncidentEffects::default())
    }
    /// Prepare exact terminal collection content under the original failure scope.
    pub(super) fn collected(
        &mut self,
        scope: IncidentOperationScope,
        evidence: CollectedIncidentEvidence,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        if evidence.capture() == CaptureEvidence::Awaiting {
            return Err(IncidentError::InvalidEvidence.into());
        }
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.current != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let failure = record.failure.as_ref().ok_or(IncidentDecisionError::WrongPhase)?;
        let summary = IncidentSummary::new(
            record.seed.clone(),
            scope.expected_revision().next()?,
            SummaryFacts::new(
                RetentionFacts::new(evidence.occurred(), None)?,
                failure.attribution,
                failure.failure,
                evidence.capture(),
                evidence.payload().clone(),
                ReportDisposition::new(0, None)?,
                evidence.symbols().clone(),
            ),
        )?;
        if let Some(Pending::Summary(expected)) = &record.pending {
            return if *expected == summary {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::ChangedDuplicate)
            };
        }
        if record.pending != Some(Pending::Collection) {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let effects = IncidentEffects::one(IncidentEffect::PublishSummary {
            scope,
            summary: summary.clone(),
        });
        let record = self.record_mut(index)?;
        record.pending = Some(Pending::Summary(summary));
        record.phase = IncidentServicePhase::SummaryPublishing;
        Ok(effects)
    }
    /// Accept exact activation before issuing source retirement and one reporter.
    pub(super) fn summary_committed(
        &mut self,
        scope: IncidentOperationScope,
        summary: IncidentSummary,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.summary.as_ref() == Some(&summary) && record.published_scope == Some(scope) {
            return Ok(IncidentEffects::default());
        }
        if record.current != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let Some(Pending::Summary(expected)) = &record.pending else {
            return Err(IncidentDecisionError::WrongPhase);
        };
        if *expected != summary {
            return Err(IncidentDecisionError::MismatchedCommit);
        }
        self.preflight_automatic_reporter()?;
        let effects =
            IncidentEffects::one(IncidentEffect::RetireSource { scope: record.source_scope });
        let record = self.record_mut(index)?;
        record.summary = Some(summary);
        record.published_scope = Some(scope);
        record.current = None;
        record.pending = None;
        record.source_requested = true;
        record.phase = IncidentServicePhase::Pending;
        Ok(effects)
    }
    /// Latch a matching failed use case without renewing its pending operation.
    pub(super) fn operation_failed(
        &mut self,
        scope: IncidentOperationScope,
        failure: IncidentServiceFailure,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        if failure == IncidentServiceFailure::SourceRetirement {
            return self.source_retirement_failed(IncidentSourceScope::Live(scope));
        }
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.current != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let expected = match record.pending.as_ref() {
            Some(Pending::Seed) => IncidentServiceFailure::SeedPublication,
            Some(Pending::Collection) => IncidentServiceFailure::Collection,
            Some(Pending::Summary(_)) => IncidentServiceFailure::SummaryPublication,
            Some(Pending::Symbols { .. }) => IncidentServiceFailure::SummaryPublication,
            Some(Pending::Attempt { .. }) => IncidentServiceFailure::AttemptPublication,
            Some(Pending::Acknowledgement { .. }) => IncidentServiceFailure::Acknowledgement,
            Some(Pending::Export(_)) => IncidentServiceFailure::Export,
            None => return Err(IncidentDecisionError::WrongPhase),
        };
        if expected != failure {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let record = self.record_mut(index)?;
        if failure == IncidentServiceFailure::Export {
            record.current = None;
            record.pending = None;
        }
        record.latch_failure(failure);
        if failure == IncidentServiceFailure::Export
            && let Some(effect) = self.activate_queued_attempt(index)?
        {
            return Ok(IncidentEffects::one(effect));
        }
        Ok(IncidentEffects::default())
    }
}
