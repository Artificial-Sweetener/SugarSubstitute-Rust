// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle the exact candidate before any independently latched payload disposition.
use super::*;
impl DiagnosticStorage {
    /// Commit or reconcile the exact candidate, then settle every selected payload.
    /// Each close and protected absence remains independently latched through errors.
    /// # Errors
    /// Unknown candidate, original expiry and partial deletion retain intent and charge.
    pub fn complete_payload_discard(
        &mut self,
        permit: &mut DiagnosticPayloadDiscardPermit,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPayloadDiscardCompletion, DiagnosticStorageError> {
        pin(&self.shared, &permit.shared, permit.current, operation)?;
        let mut state = self.check(operation, context)?;
        journal_check(&mut state, permit.id, operation.key, context)?;
        let retained = intent(&mut state, permit.id, operation.key)?;
        if retained.validating {
            return Err(error(operation.key, Code::Busy));
        }
        let decision =
            retained.decision.as_ref().ok_or_else(|| error(operation.key, Code::Closed))?;
        let (object, original, candidate, stage_id, read_id) = (
            DiagnosticObject::Summary(decision.selection.incident),
            decision.original,
            decision.candidate,
            retained.candidate_stage,
            retained.original_read,
        );
        if let Some(id) = read_id {
            close_read(&mut state, id, operation.key, context)?;
            intent(&mut state, permit.id, operation.key)?.original_read = None;
        }
        let facts = if let Some(id) = stage_id {
            settle_stage(
                &mut state,
                id,
                DiagnosticPredecessor::Exact {
                    file_identity: original.identity,
                    sha256: original.sha256,
                },
                operation.key,
                context,
            )?
        } else {
            state
                .port
                .reconcile_activation(
                    object,
                    DiagnosticPredecessor::Exact {
                        file_identity: candidate.identity,
                        sha256: candidate.sha256,
                    },
                    context,
                )
                .map_err(|cause| native(operation.key, Code::ActivationUncertain, cause))?
        };
        if !candidate.matches(facts) {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let retained = intent(&mut state, permit.id, operation.key)?;
        retained.candidate_stage = None;
        retained.candidate_active = Some(facts);
        check(&mut state, operation.key, context, false)?;
        let count = intent(&mut state, permit.id, operation.key)?.victims.len();
        for index in 0..count {
            settle_victim(&mut state, permit.id, index, operation.key, context)?;
        }
        let retained = intent(&mut state, permit.id, operation.key)?;
        let bytes = retained
            .decision
            .as_ref()
            .ok_or_else(|| error(operation.key, Code::Closed))?
            .selection
            .attachments
            .total_bytes();
        Ok(DiagnosticPayloadDiscardCompletion {
            id: permit.id,
            bytes,
            summary: DiagnosticActivation {
                shared: self.shared.clone(),
                operation: operation.key,
                object,
                facts,
            },
        })
    }
}
pub(super) fn journal_check(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let retained = intent(state, id, key)?;
    let journal = retained.journal.as_ref().ok_or_else(|| error(key, Code::Closed))?;
    if journal.closed || journal.attempted {
        return Err(error(key, Code::Busy));
    }
    let native_id = required(journal.native, key)?;
    let facts = journal.facts.ok_or_else(|| error(key, Code::Closed))?;
    if retained.journal_active != Some(facts)
        || retained.decision.as_ref().is_none_or(|decision| facts.sha256 != decision.journal_sha256)
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
fn settle_victim(
    state: &mut State,
    id: u64,
    index: usize,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let victim = &intent(state, id, key)?.victims[index];
    let object = victim.object;
    if !victim.closed {
        let native_id = required(victim.native, key)?;
        let facts = victim.facts.ok_or_else(|| error(key, Code::Closed))?;
        if !victim.issued {
            intent(state, id, key)?.victims[index].attempted = true;
            state
                .port
                .delete(native_id, object, facts, context)
                .map_err(|cause| native(key, Code::DeleteUncertain, cause))?;
            intent(state, id, key)?.victims[index].issued = true;
            check(state, key, context, false)?;
        }
        state
            .port
            .close(native_id, context)
            .map_err(|cause| native(key, Code::DeleteUncertain, cause))?;
        let victim = &mut intent(state, id, key)?.victims[index];
        victim.closed = true;
        victim.native = None;
        check(state, key, context, false)?;
    }
    if !intent(state, id, key)?.victims[index].absent {
        if !state
            .port
            .absent(object, context)
            .map_err(|cause| native(key, Code::DeleteUncertain, cause))?
        {
            return Err(error(key, Code::DeleteUncertain));
        }
        intent(state, id, key)?.victims[index].absent = true;
        state.inventory_complete = false;
        check(state, key, context, false)?;
    }
    Ok(())
}
