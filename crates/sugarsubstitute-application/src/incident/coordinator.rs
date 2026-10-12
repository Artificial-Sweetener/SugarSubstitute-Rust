// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own fixed incident slots and monotonic operation/session identities.
use super::{state::Record, *};
use sugarsubstitute_contracts::{incident::*, lifecycle::CoordinatorKey};

/// Coordinate incidents without executing native or repository operations.
pub struct IncidentCoordinator {
    pub(super) owner: CoordinatorKey,
    pub(super) records: Box<[Option<Record>]>,
    pub(super) next_operation: u64,
    pub(super) next_session: u64,
    pub(super) supervisor_seed: bool,
    pub(super) recovery: Option<IncidentRecoveryScope>,
    pub(super) recovery_closed: bool,
    pub(super) recovery_failure: Option<IncidentServiceFailure>,
}
impl IncidentCoordinator {
    /// Start the current authenticated owner with bounded empty state.
    pub fn new(owner: CoordinatorKey) -> Self {
        Self {
            owner,
            records: std::iter::repeat_with(|| None).take(MAX_INCIDENT_RECORDS).collect(),
            next_operation: 0,
            next_session: 0,
            supervisor_seed: false,
            recovery: None,
            recovery_closed: false,
            recovery_failure: None,
        }
    }
    /// Inspect matching state without manufacturing native proof.
    pub fn snapshot(&self, key: IncidentKey) -> Option<IncidentServiceSnapshot> {
        self.records.iter().flatten().find(|r| r.seed.key() == key).map(Record::snapshot)
    }
    /// Inspect original recovery correlation and actual completion/failure separately.
    pub fn recovery_snapshot(&self) -> Option<IncidentRecoverySnapshot> {
        self.recovery.map(|scope| IncidentRecoverySnapshot {
            scope,
            finished: self.recovery_closed,
            first_failure: self.recovery_failure,
        })
    }
    /// Locate the retained full subject without admitting another record.
    pub(super) fn index(&self, key: IncidentKey) -> Result<usize, IncidentDecisionError> {
        self.records
            .iter()
            .position(|r| r.as_ref().is_some_and(|r| r.seed.key() == key))
            .ok_or(IncidentDecisionError::StaleOperation)
    }
    /// Borrow exact retained state after preflight has selected its slot.
    pub(super) fn record(&self, index: usize) -> Result<&Record, IncidentDecisionError> {
        self.records[index].as_ref().ok_or(IncidentDecisionError::StaleOperation)
    }
    /// Borrow the sole matching mutable record without changing slot ownership.
    pub(super) fn record_mut(
        &mut self,
        index: usize,
    ) -> Result<&mut Record, IncidentDecisionError> {
        self.records[index].as_mut().ok_or(IncidentDecisionError::StaleOperation)
    }
    /// Select an empty or actually reclaimable fixed slot without mutation.
    pub(super) fn free_slot(&self) -> Result<usize, IncidentDecisionError> {
        self.records
            .iter()
            .position(Option::is_none)
            .or_else(|| {
                if self.recovery.is_some() && !self.recovery_closed {
                    None
                } else {
                    self.records.iter().position(|r| r.as_ref().is_some_and(Record::reclaimable))
                }
            })
            .ok_or(IncidentDecisionError::Saturated)
    }
    /// Prepare the next current logical identity without advancing its highwater.
    pub(super) fn operation(&self) -> Result<LogicalStoreOperationKey, IncidentDecisionError> {
        let next =
            self.next_operation.checked_add(1).ok_or(IncidentDecisionError::OperationExhausted)?;
        Ok(LogicalStoreOperationKey::new(self.owner, IncidentOperationId::new(next)?))
    }
    /// Prepare the next reporter session without consuming its highwater.
    pub(super) fn session(&self) -> Result<ReportSessionId, IncidentDecisionError> {
        ReportSessionId::new(
            self.next_session
                .checked_add(1)
                .ok_or(IncidentDecisionError::ReportSessionExhausted)?,
        )
        .map_err(Into::into)
    }
    /// Prepare exact current CAS correlation using the caller's original interval.
    pub(super) fn scope(
        &self,
        key: IncidentKey,
        revision: IncidentRevision,
        original: IncidentOperationBudget,
    ) -> Result<IncidentOperationScope, IncidentDecisionError> {
        Ok(IncidentOperationScope {
            incident: key,
            expected_revision: revision,
            operation: self.operation()?,
            original,
        })
    }
    /// Apply one actual host projection after validating its exact retained binding.
    /// Rejects mismatched content/scopes without altering the current operation.
    pub fn observe(
        &mut self,
        observation: IncidentServiceObservation,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        use IncidentServiceObservation::*;
        match observation {
            SeedCommitted { scope, seed } => self.seed_committed(scope, seed),
            EvidenceCollected { scope, evidence } => self.collected(scope, evidence),
            SummaryCommitted { scope, summary } => self.summary_committed(scope, summary),
            ReporterPresented { binding, context } => {
                self.record_attempt(binding, ReportAttempt::Presented, context)
            }
            ReporterFailed { binding, failure, context } => {
                self.record_attempt(binding, ReportAttempt::Failed(failure), context)
            }
            AttemptCommitted { scope, binding, summary } => {
                self.attempt_committed(scope, binding, summary)
            }
            ReportViewReplaced { binding } => self.view_replaced(binding),
            Acknowledged { scope, binding, acknowledgement, summary } => {
                self.acknowledged(scope, binding, acknowledgement, summary)
            }
            ExportFinished { scope, binding } => self.export_finished(scope, binding),
            Failed { scope, failure } => self.operation_failed(scope, failure),
            ReporterRetired { binding } => self.reporter_retired(binding),
            SourcesRetired { scope } => self.sources_retired(scope),
            SourceRetirementFailed { scope } => self.source_retirement_failed(scope),
            SeedRetired { scope } => self.seed_retired(scope),
            FamilySettled { run } => self.family_settled(run),
            SupervisorFamilySettled { owner } => self.supervisor_family_settled(owner),
            Recovered { scope, original_operation, summary } => {
                self.recovered(scope, original_operation, summary)
            }
            RecoveredSupervisorSeed { scope, original_operation, seed, actual_status, failure } => {
                self.recovered_supervisor(scope, original_operation, seed, actual_status, failure)
            }
            RecoveryFinished { scope } => self.recovery_finished(scope),
            RecoveryFailed { scope } => self.recovery_failed(scope),
        }
    }
}
