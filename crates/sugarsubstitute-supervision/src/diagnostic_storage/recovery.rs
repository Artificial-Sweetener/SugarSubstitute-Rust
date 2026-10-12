// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reconcile exact durable ledger/journal reads before independent protected absence.
mod errors;
mod stage;
mod types;
mod victim;
use super::*;
use deletion::RetainedDeletion;
pub use errors::{DiagnosticRecoveryValidationCode, DiagnosticRecoveryValidationError};
use source::{canonical, retained_source, whole};
pub use types::{
    DiagnosticRecoveryDecision, DiagnosticRecoveryLedgerCase, DiagnosticRecoverySource,
    DiagnosticRecoveryValidator,
};
pub use victim::DiagnosticRecoveredVictimState;

impl DiagnosticStorage {
    /// Recover summary-eviction authority from complete actual journal and ledger reads.
    ///
    /// The trusted codec adapter validates semantic original/candidate facts. Storage
    /// independently binds their exact native sources and selected protected victim.
    /// Success transfers journal custody and checked-closes the ledger lease; an exact
    /// present victim remains retained for the one subsequent deletion lease.
    /// # Errors
    /// Unknown/third ledger, foreign sources, changed identity, Candidate+present,
    /// failed reads/probes/close or original context expiry preserve native custody.
    pub fn recover_disposition(
        &mut self,
        journal: &mut DiagnosticReadLease,
        ledger: &mut DiagnosticReadLease,
        validator: &dyn DiagnosticRecoveryValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DurableDeletionPermit, DiagnosticStorageError> {
        if journal.id == ledger.id {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let journal_source = whole(self, journal, operation, context)?;
        let ledger_source = whole(self, ledger, operation, context)?;
        let decision = validator.validate(operation.key, &journal_source, &ledger_source).map_err(
            |cause| {
                native(
                    operation.key,
                    Code::InvalidIdentity,
                    ClaimError::new("diagnostic_recovery_validation", cause),
                )
            },
        )?;
        let mut state = self.check(operation, context)?;
        super::stage::mutation(&state, operation.key)?;
        let plan = decision.plan;
        let namespace = state.namespace.ok_or_else(|| error(operation.key, Code::Closed))?;
        if plan.operation.owner().namespace != namespace
            || state.deletion.is_some()
            || !canonical(&journal_source, DiagnosticObject::Journal(plan.operation), namespace)
            || !canonical(&ledger_source, DiagnosticObject::StoreState, namespace)
            || journal_source.facts.sha256 != plan.journal_sha256
            || decision.candidate_ledger_sha256 != plan.completion_sha256
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let selected = match decision.ledger_case {
            DiagnosticRecoveryLedgerCase::Original => decision.original_ledger_sha256,
            DiagnosticRecoveryLedgerCase::Candidate => decision.candidate_ledger_sha256,
        };
        if ledger_source.facts.sha256 != selected
            || (decision.ledger_case == DiagnosticRecoveryLedgerCase::Candidate
                && !decision.candidate_stage.matches(ledger_source.facts))
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let journal_index = retained_source(&mut state, &journal_source, operation.key, context)?;
        retained_source(&mut state, &ledger_source, operation.key, context)?;
        let id = next_lease(&mut state, operation.key)?;
        state.deletion = Some(RetainedDeletion {
            id,
            plan,
            journal: Some((journal_source.native, journal_source.facts)),
            victim: None,
            victim_facts: None,
            lease_issued: false,
            victim_settled: false,
            recovery: Some((decision.ledger_case, decision.candidate_stage, ledger_source.facts)),
        });
        state.reads.remove(journal_index);
        let absent = state
            .port
            .absent(plan.victim, context)
            .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
        check(&mut state, operation.key, context, false)?;
        if !absent {
            if decision.ledger_case == DiagnosticRecoveryLedgerCase::Candidate {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            let (native_id, facts) = state
                .port
                .open(plan.victim, plan.victim.bound(), context)
                .map_err(|cause| native(operation.key, Code::Read, cause))?;
            let intent =
                state.deletion.as_mut().ok_or_else(|| error(operation.key, Code::Closed))?;
            intent.victim = Some(native_id);
            intent.victim_facts = Some(facts);
            if native_id == 0 || !plan.facts.matches(facts) {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            check(&mut state, operation.key, context, false)?;
        } else {
            state
                .deletion
                .as_mut()
                .ok_or_else(|| error(operation.key, Code::Closed))?
                .victim_settled = true;
        }
        let ledger_index = retained_source(&mut state, &ledger_source, operation.key, context)?;
        state
            .port
            .close(ledger_source.native, context)
            .map_err(|cause| native(operation.key, Code::Retirement, cause))?;
        check(&mut state, operation.key, context, false)?;
        state.reads.remove(ledger_index);
        Ok(DurableDeletionPermit { shared: self.shared.clone(), operation: operation.key, id })
    }
}
