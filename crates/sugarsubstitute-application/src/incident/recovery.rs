// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate bounded historical projections with one current original recovery admission.
use super::{
    state::{FailureInput, Pending, Record},
    *,
};
use sugarsubstitute_contracts::incident::*;

impl IncidentCoordinator {
    /// Begin one current recovery operation; exact retry retains the original interval.
    /// A completed recovery cannot be reopened and exhaustion precedes any effects.
    pub fn begin_recovery(
        &mut self,
        original: IncidentOperationBudget,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        if let Some(scope) = self.recovery {
            return if !self.recovery_closed && scope.original() == original {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::StaleOperation)
            };
        }
        let scope = IncidentRecoveryScope { operation: self.operation()?, original };
        let effects = IncidentEffects::one(IncidentEffect::Recover { scope });
        self.next_operation = scope.operation().id().get();
        self.recovery = Some(scope);
        Ok(effects)
    }
    /// Validate current admission and inert historical owner facts before projection.
    fn check_recovery(
        &self,
        scope: IncidentRecoveryScope,
        original: LogicalStoreOperationKey,
        key: IncidentKey,
    ) -> Result<(), IncidentDecisionError> {
        if self.recovery != Some(scope) || self.recovery_closed {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if original.owner().namespace != self.owner.namespace
            || key.subject().coordinator().namespace != self.owner.namespace
        {
            return Err(IncidentDecisionError::ForeignNamespace);
        }
        if original.owner() != key.subject().coordinator() {
            return Err(IncidentDecisionError::MismatchedCommit);
        }
        Ok(())
    }
    /// Prepare bounded historical state without inventing an original monotonic interval.
    fn recovered_record(
        seed: IncidentSeed,
        scope: IncidentRecoveryScope,
        original: LogicalStoreOperationKey,
        revision: IncidentRevision,
    ) -> Record {
        let source_scope = IncidentSourceScope::Recovered(RecoveredSourceScope {
            recovery: scope,
            historical_operation: original,
            incident: seed.key(),
            expected_revision: revision,
        });
        Record {
            seed,
            seed_context: None,
            seed_scope: None,
            seed_committed: true,
            original_publication: original,
            source_scope,
            current: None,
            pending: None,
            failure: None,
            failure_context: None,
            summary: None,
            recovery_projection: None,
            published_scope: None,
            last_commit: None,
            phase: IncidentServicePhase::Pending,
            binding: None,
            automatic_report_attempted: false,
            presented: false,
            attempted_session: false,
            last_attempt: None,
            queued_failure: None,
            refresh_pending: false,
            reporter_retired: true,
            sources_retired: false,
            source_requested: false,
            source_failed: false,
            family_settled: false,
            clean_retirement: false,
            acknowledgement: None,
            restart_requested: false,
            restart_forwarded: false,
            first_failure: None,
        }
    }
    /// Admit one exact validated historical summary under current recovery admission.
    pub(super) fn recovered(
        &mut self,
        scope: IncidentRecoveryScope,
        original: LogicalStoreOperationKey,
        summary: IncidentSummary,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let key = summary.seed().key();
        self.check_recovery(scope, original, key)?;
        if matches!(
            summary.facts().exit(),
            ExitAttribution::Desktop {
                kind: sugarsubstitute_contracts::lifecycle::DesktopExitKind::Normal
                    | sugarsubstitute_contracts::lifecycle::DesktopExitKind::StartupCancelled,
                ..
            }
        ) {
            return Err(IncidentDecisionError::WrongPhase);
        }
        if let Ok(index) = self.index(key) {
            let record = self.record(index)?;
            return if record.recovery_projection.as_ref() == Some(&summary)
                && record.original_publication == original
                && matches!(record.source_scope,IncidentSourceScope::Recovered(source) if source.recovery()==scope)
            {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::ChangedDuplicate)
            };
        }
        let index = self.free_slot()?;
        let mut record =
            Self::recovered_record(summary.seed().clone(), scope, original, summary.revision());
        record.failure = Some(FailureInput {
            fact: None,
            attribution: summary.facts().exit(),
            failure: summary.facts().failure(),
        });
        record.source_requested = true;
        let effects = if let Some(at) = summary.facts().retention().acknowledged() {
            record.acknowledgement = Some(DurableAcknowledgement::new(key, summary.revision(), at));
            record.phase = IncidentServicePhase::Acknowledged;
            IncidentEffects::one(IncidentEffect::RetireSource { scope: record.source_scope })
        } else {
            let session = self.session()?;
            let binding = ReportBinding::new(key, summary.revision(), session);
            let effects = IncidentEffects::pair(
                IncidentEffect::RetireSource { scope: record.source_scope },
                IncidentEffect::LaunchReporter { binding, summary: summary.clone() },
            );
            record.binding = Some(binding);
            record.automatic_report_attempted = true;
            record.reporter_retired = false;
            record.phase = IncidentServicePhase::Reporting;
            self.next_session = session.get();
            effects
        };
        record.recovery_projection = Some(summary.clone());
        record.summary = Some(summary);
        self.records[index] = Some(record);
        Ok(effects)
    }
    /// Collect externally established supervisor failure under the current original budget.
    pub(super) fn recovered_supervisor(
        &mut self,
        scope: IncidentRecoveryScope,
        original: LogicalStoreOperationKey,
        seed: IncidentSeed,
        status: i64,
        failure: FailureEvidence,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let key = seed.key();
        self.check_recovery(scope, original, key)?;
        if !matches!(key.subject(), IncidentSubject::Supervisor(_)) {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let input = FailureInput {
            fact: None,
            attribution: ExitAttribution::Supervisor { status },
            failure,
        };
        if let Ok(index) = self.index(key) {
            let record = self.record(index)?;
            return if record.seed == seed
                && record.original_publication == original
                && record.failure.as_ref() == Some(&input)
                && matches!(record.source_scope,IncidentSourceScope::Recovered(source) if source.recovery()==scope)
            {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::ChangedDuplicate)
            };
        }
        let index = self.free_slot()?;
        seed.revision().next()?;
        let current = self.scope(key, seed.revision(), scope.original())?;
        let effects = IncidentEffects::one(IncidentEffect::CollectEvidence {
            scope: current,
            seed: seed.clone(),
            attribution: input.attribution,
            failure,
        });
        let mut record = Self::recovered_record(seed, scope, original, current.expected_revision());
        record.failure = Some(input);
        record.current = Some(current);
        record.pending = Some(Pending::Collection);
        record.phase = IncidentServicePhase::Collecting;
        self.next_operation = current.operation().id().get();
        self.records[index] = Some(record);
        Ok(effects)
    }
    /// Close new recovery projections while retaining outstanding source correlations.
    pub(super) fn recovery_finished(
        &mut self,
        scope: IncidentRecoveryScope,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        if self.recovery != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        self.recovery_closed = true;
        Ok(IncidentEffects::default())
    }
    /// Latch the original current recovery failure without closing or renewing admission.
    pub(super) fn recovery_failed(
        &mut self,
        scope: IncidentRecoveryScope,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        if self.recovery != Some(scope) || self.recovery_closed && self.recovery_failure.is_none() {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if self.recovery_failure.is_none() {
            self.recovery_failure = Some(IncidentServiceFailure::Recovery);
        }
        Ok(IncidentEffects::default())
    }
}
