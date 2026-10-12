// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Activate only the sealed create-only Journal and retain checked native custody.
use super::*;
impl DiagnosticStorage {
    /// Commit the exact staged Journal before granting candidate or payload effects.
    /// # Errors
    /// Foreign pins, altered bytes and uncertain rename/open/readback retain custody.
    pub fn activate_payload_discard(
        &mut self,
        preparation: &mut DiagnosticPayloadDiscardPreparation,
        stage: &mut DiagnosticPayloadDiscardStage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPayloadDiscardPermit, DiagnosticStorageError> {
        pin(&self.shared, &preparation.shared, preparation.current, operation)?;
        pin(&self.shared, &stage.shared, stage.current, operation)?;
        if stage.id != preparation.id {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let mut state = self.check(operation, context)?;
        let retained = intent(&mut state, stage.id, operation.key)?;
        if retained.validating || retained.decision.is_none() {
            return Err(error(operation.key, Code::Busy));
        }
        if retained.candidate_active.is_none() {
            let candidate = required(retained.candidate_stage, operation.key)?;
            let expected = retained
                .decision
                .as_ref()
                .ok_or_else(|| error(operation.key, Code::Closed))?
                .candidate;
            let native_id = required(
                state.stages[stage_index(&state, candidate, operation.key)?].native,
                operation.key,
            )?;
            let actual = state
                .port
                .observe_file(native_id, context)
                .map_err(|cause| native(operation.key, Code::Read, cause))?;
            if !expected.matches(actual) {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            check(&mut state, operation.key, context, false)?;
            revalidate_victims(&mut state, stage.id, operation.key, context)?;
        }
        let retained = intent(&mut state, stage.id, operation.key)?;
        if retained.journal_active.is_none() {
            let stage_id = required(retained.journal_stage, operation.key)?;
            let expected = retained
                .decision
                .as_ref()
                .ok_or_else(|| error(operation.key, Code::Closed))?
                .journal_sha256;
            if stage_facts(&state, stage_id, operation.key)?.sha256 != expected {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            let facts = settle_stage(
                &mut state,
                stage_id,
                DiagnosticPredecessor::Absent,
                operation.key,
                context,
            )?;
            let retained = intent(&mut state, stage.id, operation.key)?;
            retained.journal_stage = None;
            retained.journal_active = Some(facts);
        }
        adopt_journal(&mut state, stage.id, operation.key, context)?;
        Ok(DiagnosticPayloadDiscardPermit {
            shared: self.shared.clone(),
            current: operation.key,
            id: stage.id,
        })
    }
}
pub(super) fn adopt_journal(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    let retained = intent(state, id, key)?;
    let decision = retained.decision.as_ref().ok_or_else(|| error(key, Code::Closed))?;
    let object = DiagnosticObject::Journal(decision.historical);
    if retained.journal.is_none() {
        retained.journal = Some(RetainedVictim::new(object));
    }
    if intent(state, id, key)?.journal.as_ref().is_none_or(|journal| journal.native.is_none()) {
        check(state, key, context, false)?;
        let (native_id, facts) = state
            .port
            .open(object, DIAGNOSTIC_IO_BYTES as u64, context)
            .map_err(|cause| native(key, Code::Read, cause))?;
        let journal =
            intent(state, id, key)?.journal.as_mut().ok_or_else(|| error(key, Code::Closed))?;
        journal.native = Some(native_id);
        journal.facts = Some(facts);
    }
    let retained = intent(state, id, key)?;
    let journal = retained.journal.as_ref().ok_or_else(|| error(key, Code::Closed))?;
    let native_id = required(journal.native, key)?;
    let facts = journal.facts.ok_or_else(|| error(key, Code::Closed))?;
    if native_id == 0
        || retained.journal_active != Some(facts)
        || retained.decision.as_ref().is_none_or(|decision| decision.journal_sha256 != facts.sha256)
        || retained
            .victims
            .iter()
            .any(|victim| victim.facts.is_some_and(|actual| actual.identity == facts.identity))
        || retained.decision.as_ref().is_some_and(|decision| {
            decision.original.identity == facts.identity
                || decision.candidate.identity == facts.identity
        })
    {
        return Err(error(key, Code::ChangedIdentity));
    }
    let actual = state
        .port
        .observe_file(native_id, context)
        .map_err(|cause| native(key, Code::Read, cause))?;
    if actual != facts {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)
}
