// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire the exact Journal only after matching candidate and every victim settlement.
use super::*;
impl DiagnosticStorage {
    /// Verify the same completion, then delete, close and observe Journal absence.
    /// This never increments Summary-eviction counters or deletes a different group.
    /// # Errors
    /// Changed completion/current Summary and failed Journal effects retain actual state.
    pub fn finish_payload_discard(
        &mut self,
        permit: &mut DiagnosticPayloadDiscardPermit,
        completion: &DiagnosticPayloadDiscardCompletion,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticDeletion, DiagnosticStorageError> {
        pin(&self.shared, &permit.shared, permit.current, operation)?;
        pin(&self.shared, &completion.summary.shared, completion.summary.operation, operation)?;
        let mut state = self.check(operation, context)?;
        let retained = intent(&mut state, permit.id, operation.key)?;
        let decision =
            retained.decision.as_ref().ok_or_else(|| error(operation.key, Code::Closed))?;
        if retained.validating
            || completion.id != permit.id
            || retained.candidate_active != Some(completion.summary.facts)
            || completion.summary.object != DiagnosticObject::Summary(decision.selection.incident)
            || !decision.candidate.matches(completion.summary.facts)
            || completion.bytes != decision.selection.attachments.total_bytes()
            || retained
                .victims
                .iter()
                .any(|victim| !victim.closed || !victim.absent || victim.native.is_some())
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let actual = state
            .port
            .reconcile_activation(
                completion.summary.object,
                predecessor(completion.summary.facts),
                context,
            )
            .map_err(|cause| native(operation.key, Code::ActivationUncertain, cause))?;
        if actual != completion.summary.facts {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        check(&mut state, operation.key, context, false)?;
        let retained = intent(&mut state, permit.id, operation.key)?;
        let journal =
            retained.journal.as_ref().ok_or_else(|| error(operation.key, Code::Closed))?;
        let object = journal.object;
        let facts = journal.facts.ok_or_else(|| error(operation.key, Code::Closed))?;
        if !journal.closed {
            let native_id = required(journal.native, operation.key)?;
            if !journal.issued {
                intent(&mut state, permit.id, operation.key)?
                    .journal
                    .as_mut()
                    .ok_or_else(|| error(operation.key, Code::Closed))?
                    .attempted = true;
                state
                    .port
                    .delete(native_id, object, facts, context)
                    .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
                intent(&mut state, permit.id, operation.key)?
                    .journal
                    .as_mut()
                    .ok_or_else(|| error(operation.key, Code::Closed))?
                    .issued = true;
                check(&mut state, operation.key, context, false)?;
            }
            state
                .port
                .close(native_id, context)
                .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?;
            let journal = intent(&mut state, permit.id, operation.key)?
                .journal
                .as_mut()
                .ok_or_else(|| error(operation.key, Code::Closed))?;
            journal.closed = true;
            journal.native = None;
            check(&mut state, operation.key, context, false)?;
        }
        if !state
            .port
            .absent(object, context)
            .map_err(|cause| native(operation.key, Code::DeleteUncertain, cause))?
        {
            return Err(error(operation.key, Code::DeleteUncertain));
        }
        intent(&mut state, permit.id, operation.key)?
            .journal
            .as_mut()
            .ok_or_else(|| error(operation.key, Code::Closed))?
            .absent = true;
        check(&mut state, operation.key, context, false)?;
        state.payload = None;
        state.inventory_complete = false;
        Ok(DiagnosticDeletion { object, facts })
    }
}
