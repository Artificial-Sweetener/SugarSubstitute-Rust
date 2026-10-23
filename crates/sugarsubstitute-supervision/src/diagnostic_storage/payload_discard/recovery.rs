// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover actual historical Journal and Summary under fresh sealed current authority.
use super::*;
impl DiagnosticStorage {
    /// Validate actual whole historical sources, then adopt exact existing victims.
    /// Original recovery requires every victim present; Candidate permits protected absence.
    /// # Errors
    /// Third states, incomplete inventory and partial outputs preserve all actual custody.
    pub fn recover_payload_discard(
        &mut self,
        journal: &mut DiagnosticReadLease,
        summary: &mut DiagnosticReadLease,
        validator: &dyn DiagnosticPayloadDiscardValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPayloadDiscardPermit, DiagnosticStorageError> {
        pin(&self.shared, &journal.shared, journal.operation, operation)?;
        pin(&self.shared, &summary.shared, summary.operation, operation)?;
        let id = {
            let mut state = self.check(operation, context)?;
            stage::mutation(&mut state, operation.key)?;
            capacity(&mut state, operation.key)?;
            if state.data_mut()?.deletion.is_some() {
                return Err(error(operation.key, Code::Busy));
            }
            let intent_allocation =
                allocation::payload_intent(&state.data_mut()?.allocation, operation.key)?;
            let reservation =
                allocation::reservation(&state.data_mut()?.allocation, Some(operation.key))?;
            let mut victims = Vec::new();
            victims
                .try_reserve_exact(MAX_ATTACHMENTS)
                .map_err(|cause| reservation.failed(cause))?;
            let selected = state
                .data_mut()?
                .reads
                .iter()
                .find(|read| read.id == summary.id)
                .ok_or_else(|| error(operation.key, Code::Closed))?;
            let subject_digest = match (selected.object, selected.candidate) {
                (Some(DiagnosticObject::Summary(key)), _) => {
                    crate::durable_storage::protected::record_file::diagnostic_subject_digest(key)
                }
                (None, Some((DiagnosticEntryClass::Summary, digest))) => digest,
                _ => return Err(error(operation.key, Code::InvalidIdentity)),
            };
            if state.data_mut()?.reads.iter().any(|read| {
                read.id != summary.id
                    && read.id != journal.id
                    && (read.object.is_some_and(|object| {
                        crate::durable_storage::protected::record_file::diagnostic_object_binding(
                            object,
                            operation.key.owner().namespace,
                        )
                        .1 == subject_digest
                    }) || read.candidate.is_some_and(|(_, digest)| digest == subject_digest))
            }) {
                return Err(error(operation.key, Code::Busy));
            }
            let id = next_lease(&mut state, operation.key)?;
            state.data_mut()?.payload = Some(intent_allocation.payload_intent(PayloadIntent {
                id,
                subject_digest,
                reserved_files: MAX_ATTACHMENTS + 2,
                selection: None,
                original_read: Some(summary.id),
                pending_journal_read: Some(journal.id),
                candidate_stage: None,
                journal_stage: None,
                journal: None,
                victims,
                validating: true,
                decision: None,
                journal_bytes: None,
                journal_active: None,
                candidate_active: None,
            }));
            id
        };
        let journal_source = whole(&self.shared, journal, operation, context)?;
        let summary_source = whole(&self.shared, summary, operation, context)?;
        let destination = self.payload_decision(operation, context)?;
        let decision = allocation::decode_callback(self, operation, context, |decode| {
            validator.validate_recovery(
                decode,
                destination,
                operation.key,
                &journal_source,
                &summary_source,
            )
        })?;
        let mut state = self.check(operation, context)?;
        let journal_index = retained_source(&mut state, &journal_source, operation.key, context)?;
        retained_source(&mut state, &summary_source, operation.key, context)?;
        let namespace = operation.key.owner().namespace;
        let expected = match decision.case {
            DiagnosticPayloadDiscardCase::Original => decision.original,
            DiagnosticPayloadDiscardCase::Candidate => decision.candidate,
        };
        if !decision._allocation.matches(operation)
            || decision.historical.owner().namespace != namespace
            || !canonical(
                &journal_source,
                DiagnosticObject::Journal(decision.historical),
                namespace,
            )
            || !canonical(
                &summary_source,
                DiagnosticObject::Summary(decision.selection.incident),
                namespace,
            )
            || journal_source.facts.sha256 != decision.journal_sha256
            || !expected.matches(summary_source.facts)
            || decision
                .victims
                .iter()
                .any(|victim| victim.facts.identity == journal_source.facts.identity)
            || decision.original.identity == journal_source.facts.identity
            || decision.candidate.identity == journal_source.facts.identity
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let retained = state.data_mut()?.reads.remove(journal_index);
        let owner = intent(&mut state, id, operation.key)?;
        owner.journal = Some(RetainedVictim {
            object: DiagnosticObject::Journal(decision.historical),
            native: retained.native,
            facts: retained.facts,
            attempted: false,
            issued: false,
            closed: false,
            absent: false,
        });
        owner.pending_journal_read = None;
        owner.journal_active = Some(journal_source.facts);
        owner.selection = Some(decision.selection.clone());
        owner
            .victims
            .extend(decision.victims.iter().map(|victim| RetainedVictim::new(victim.object)));
        owner.validating = false;
        owner.decision = Some(decision);
        adopt_victims(&mut state, id, operation.key, context)?;
        intent(&mut state, id, operation.key)?.reserved_files = 2;
        if intent(&mut state, id, operation.key)?
            .decision
            .as_ref()
            .is_some_and(|decision| decision.case == DiagnosticPayloadDiscardCase::Original)
        {
            adopt_candidate(&mut state, id, operation.key, context)?;
        }
        intent(&mut state, id, operation.key)?.reserved_files = 1;
        Ok(DiagnosticPayloadDiscardPermit {
            shared: self.shared.clone(),
            current: operation.key,
            id,
        })
    }
}
fn adopt_victims(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    let count = intent(state, id, key)?.victims.len();
    for index in 0..count {
        check(state, key, context, false)?;
        let owner = intent(state, id, key)?;
        let decision = owner.decision.as_ref().ok_or_else(|| error(key, Code::Closed))?;
        let (expected, case) = (decision.victims[index], decision.case);
        if state
            .call_native(|port| port.absent(expected.object, context))?
            .map_err(|cause| native(key, Code::Read, cause))?
        {
            if case == DiagnosticPayloadDiscardCase::Original {
                return Err(error(key, Code::ChangedIdentity));
            }
            let victim = &mut intent(state, id, key)?.victims[index];
            victim.closed = true;
            victim.absent = true;
            check(state, key, context, false)?;
            continue;
        }
        check(state, key, context, false)?;
        let (native_id, facts) = state
            .call_native(|port| port.open(expected.object, expected.object.bound(), context))?
            .map_err(|cause| native(key, Code::Read, cause))?;
        let owner = intent(state, id, key)?;
        owner.victims[index].native = Some(native_id);
        owner.victims[index].facts = Some(facts);
        if native_id == 0 || !expected.facts.matches(facts) {
            return Err(error(key, Code::ChangedIdentity));
        }
        check(state, key, context, false)?;
    }
    Ok(())
}
fn adopt_candidate(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    if !state.data_mut()?.inventory_complete {
        return Err(error(key, Code::InventoryIncomplete));
    }
    let decision =
        intent(state, id, key)?.decision.as_ref().ok_or_else(|| error(key, Code::Closed))?;
    let (object, historical, expected) = (
        DiagnosticObject::Summary(decision.selection.incident),
        decision.historical,
        decision.candidate,
    );
    let stage_id = reserve_stage(state, object, stage::StageRole::PayloadCandidate, 0, key)?;
    intent(state, id, key)?.candidate_stage = Some(stage_id);
    let index = stage_index(state, stage_id, key)?;
    state.data_mut()?.stages[index].recovered = Some(expected);
    state.data_mut()?.stages[index].written = expected.bytes;
    let (native_id, actual) = state
        .call_native(|port| {
            port.recover_summary_stage(
                object,
                historical,
                expected,
                SummaryStageAccess::Candidate,
                context,
            )
        })?
        .map_err(|cause| native(key, Code::Read, cause))?;
    state.data_mut()?.stages[index].native = Some(native_id);
    if native_id == 0 || !expected.matches(actual) {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    state.data_mut()?.stages[index].flushed = Some(actual);
    Ok(())
}
