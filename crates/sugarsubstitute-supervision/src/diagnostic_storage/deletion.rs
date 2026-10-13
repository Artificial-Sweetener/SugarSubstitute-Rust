// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require a retained actual durable journal before exact by-handle disposition.
use super::*;

pub(super) struct RetainedDeletion {
    pub(super) id: u64,
    pub(super) plan: DiagnosticDispositionPlan,
    pub(super) journal: Option<(NativeId, DiagnosticReadFacts)>,
    pub(super) victim: Option<NativeId>,
    pub(super) victim_settled: bool,
    pub(super) victim_facts: Option<DiagnosticReadFacts>,
    pub(super) lease_issued: bool,
    pub(super) recovery:
        Option<(DiagnosticRecoveryLedgerCase, RecordedDiagnosticFileFacts, DiagnosticReadFacts)>,
}
/// Borrow one actual committed victim-bound journal; public facts cannot mint it.
pub struct DurableDeletionPermit {
    pub(super) shared: Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) id: u64,
}
/// Pin the exact deletion-capable victim without granting permit-free mutation.
pub struct DiagnosticDeleteLease {
    shared: Shared,
    operation: LogicalStoreOperationKey,
    id: u64,
}
/// Observe checked by-handle deletion, final closure and protected absence.
pub struct DiagnosticDeletion {
    pub(super) object: DiagnosticObject,
    pub(super) facts: DiagnosticReadFacts,
}
impl DiagnosticDeletion {
    /// Observe the exact durably disposed object.
    pub const fn object(&self) -> DiagnosticObject {
        self.object
    }
    /// Observe the actual former source identity, digest and length.
    pub const fn facts(&self) -> DiagnosticReadFacts {
        self.facts
    }
}
impl DiagnosticStorage {
    /// Commit a checked exact journal and retain its actual read custody before minting.
    /// # Errors
    /// Any activation/open/readback failure retains the intent and actual outputs.
    pub fn activate_disposition(
        &mut self,
        stage: &mut DiagnosticDispositionStage,
        expected: DiagnosticPredecessor,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DurableDeletionPermit, DiagnosticStorageError> {
        {
            let mut state = self.check(operation, context)?;
            if stage.plan.operation != operation.key
                || state.data_mut()?.deletion.is_some()
                || state.data_mut()?.migration.is_some()
                || state.data_mut()?.payload.is_some()
                || !Shared::ptr_eq(&stage.stage.shared, &self.shared)
            {
                return Err(error(operation.key, Code::InvalidIdentity));
            }
            let flushed = state
                .data_mut()?
                .stages
                .iter()
                .find(|value| value.id == stage.stage.id)
                .and_then(|value| value.flushed)
                .ok_or_else(|| error(operation.key, Code::Flush))?;
            if flushed.sha256 != stage.plan.journal_sha256 {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            let id = next_lease(&mut state, operation.key)?;
            state.data_mut()?.deletion = Some(RetainedDeletion {
                id,
                plan: stage.plan,
                journal: None,
                victim: None,
                victim_settled: false,
                victim_facts: None,
                lease_issued: false,
                recovery: None,
            });
        }
        let active = self.activate(&mut stage.stage, expected, operation, context)?;
        let mut state = self.check(operation, context)?;
        let (id, facts) = state
            .call_native(|port| port.open(active.object, 65536, context))?
            .map_err(|cause| native(operation.key, Code::Read, cause))?;
        let intent = state
            .data_mut()?
            .deletion
            .as_mut()
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        intent.journal = Some((id, facts));
        if facts != active.facts || id == 0 {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let permit_id = intent.id;
        check(&mut state, operation.key, context, false)?;
        Ok(DurableDeletionPermit {
            shared: self.shared.clone(),
            operation: operation.key,
            id: permit_id,
        })
    }
    /// Retain the exact DELETE-capable victim while excluding all read/export pins.
    /// # Errors
    /// Foreign permits, duplicate intent, live reads, or changed victim facts reject.
    pub fn prepare_delete(
        &mut self,
        permit: &mut DurableDeletionPermit,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticDeleteLease, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        permit_check(&mut state, &self.shared, permit, operation.key)?;
        stage::mutation(&mut state, operation.key)?;
        let retained = state
            .data_mut()?
            .deletion
            .as_ref()
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        let plan = retained.plan;
        let missing_victim = retained.victim.is_none();
        if retained.lease_issued
            || retained.victim_settled
            || state.data_mut()?.reads.iter().any(|value| value.object == Some(plan.victim))
        {
            return Err(error(operation.key, Code::Busy));
        }
        if missing_victim {
            let (id, facts) = state
                .call_native(|port| port.open(plan.victim, plan.victim.bound(), context))?
                .map_err(|cause| native(operation.key, Code::Read, cause))?;
            let retained = state
                .data_mut()?
                .deletion
                .as_mut()
                .ok_or_else(|| error(operation.key, Code::Closed))?;
            retained.victim = Some(id);
            retained.victim_facts = Some(facts);
        }
        let retained = state
            .data_mut()?
            .deletion
            .as_ref()
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        if retained.victim == Some(0)
            || retained.victim_facts.is_none_or(|facts| !plan.facts.matches(facts))
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        check(&mut state, operation.key, context, false)?;
        state
            .data_mut()?
            .deletion
            .as_mut()
            .ok_or_else(|| error(operation.key, Code::Closed))?
            .lease_issued = true;
        Ok(DiagnosticDeleteLease {
            shared: self.shared.clone(),
            operation: operation.key,
            id: permit.id,
        })
    }
    /// Apply exact by-handle disposition and prove actual absence after final closure.
    /// # Errors
    /// Failed mutation/close/absence keeps intent and byte charge; repeats can settle
    /// the same retained actual handle but cannot select another victim.
    pub fn delete(
        &mut self,
        lease: &mut DiagnosticDeleteLease,
        permit: &mut DurableDeletionPermit,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticDeletion, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        permit_check(&mut state, &self.shared, permit, operation.key)?;
        stage::mutation(&mut state, operation.key)?;
        if !Shared::ptr_eq(&self.shared, &lease.shared)
            || lease.operation != operation.key
            || lease.id != permit.id
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let retained = state
            .data_mut()?
            .deletion
            .as_ref()
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        let plan = retained.plan;
        let actual = retained.victim_facts.ok_or_else(|| error(operation.key, Code::Closed))?;
        if !plan.facts.matches(actual) {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        if !retained.victim_settled {
            let id = retained.victim.ok_or_else(|| error(operation.key, Code::Closed))?;
            state
                .call_native(|port| port.delete(id, plan.victim, actual, context))?
                .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
            state
                .call_native(|port| port.close(id, context))?
                .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
            if !state
                .call_native(|port| port.absent(plan.victim, context))?
                .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?
            {
                return Err(error(operation.key, Code::DeleteUncertain));
            }
            check(&mut state, operation.key, context, false)?;
            let retained = state
                .data_mut()?
                .deletion
                .as_mut()
                .ok_or_else(|| error(operation.key, Code::Closed))?;
            retained.victim = None;
            retained.victim_settled = true;
            state.data_mut()?.inventory_complete = false;
        }
        Ok(DiagnosticDeletion { object: plan.victim, facts: actual })
    }
    /// Retire the exact journal only after victim settlement and actual matching completion.
    /// # Errors
    /// A public summary, exit code or foreign/changed activation cannot substitute
    /// for the private actual completion proof; failed journal close retains custody.
    pub fn retire_disposition(
        &mut self,
        permit: &mut DurableDeletionPermit,
        completion: &DiagnosticActivation,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticDeletion, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        permit_check(&mut state, &self.shared, permit, operation.key)?;
        stage::mutation(&mut state, operation.key)?;
        if !state.data_mut()?.writer
            || !Shared::ptr_eq(&completion.shared, &self.shared)
            || completion.operation != operation.key
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let retained = state
            .data_mut()?
            .deletion
            .as_ref()
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        let plan = retained.plan;
        if !retained.victim_settled
            || completion.object != plan.completion
            || completion.facts.sha256 != plan.completion_sha256
            || retained.recovery.is_some_and(|(_, expected, _)| !expected.matches(completion.facts))
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let (id, facts) = retained.journal.ok_or_else(|| error(operation.key, Code::Closed))?;
        let object = DiagnosticObject::Journal(plan.operation);
        state
            .call_native(|port| port.delete(id, object, facts, context))?
            .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
        state
            .call_native(|port| port.close(id, context))?
            .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
        if !state
            .call_native(|port| port.absent(object, context))?
            .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?
        {
            return Err(error(operation.key, Code::DeleteUncertain));
        }
        check(&mut state, operation.key, context, false)?;
        state.data_mut()?.deletion = None;
        state.data_mut()?.inventory_complete = false;
        Ok(DiagnosticDeletion { object, facts })
    }
}
pub(super) fn permit_check(
    state: &mut StateAccess<'_>,
    shared: &Shared,
    permit: &DurableDeletionPermit,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if !Shared::ptr_eq(shared, &permit.shared)
        || permit.operation != key
        || state
            .data_mut()?
            .deletion
            .as_ref()
            .is_none_or(|value| value.id != permit.id || value.journal.is_none())
    {
        return Err(error(key, Code::InvalidIdentity));
    }
    Ok(())
}
