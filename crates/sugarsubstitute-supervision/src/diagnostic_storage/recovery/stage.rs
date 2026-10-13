// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover only the original completion inode pinned by an actual durable intent.
use super::*;
impl DiagnosticStorage {
    /// Retain the exact historical candidate stage after actual victim settlement.
    ///
    /// Refresh the complete inventory after deletion before this call. Recovery
    /// keeps existing physical bytes charged and reserves one of the eight slots;
    /// it creates no file and returned candidate bytes cannot be rewritten.
    /// # Errors
    /// Rejects foreign/current authority, Candidate state, unsettled victim,
    /// incomplete inventory, missing/ambiguous/changed stage or expired context.
    /// Partial native outputs remain owner-retained even when no pin is returned.
    pub fn recover_completion_stage(
        &mut self,
        permit: &mut DurableDeletionPermit,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStage, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        deletion::permit_check(&mut state, &self.shared, permit, operation.key)?;
        super::super::stage::mutation(&mut state, operation.key)?;
        let intent = state
            .data_mut()?
            .deletion
            .as_ref()
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        let (case, expected, original) =
            intent.recovery.ok_or_else(|| error(operation.key, Code::InvalidIdentity))?;
        let plan = intent.plan;
        if case != DiagnosticRecoveryLedgerCase::Original
            || !intent.victim_settled
            || state.data_mut()?.stages.len() >= DIAGNOSTIC_STAGE_BOUND
            || state.data_mut()?.stages.iter().any(|stage| stage.recovered == Some(expected))
        {
            return Err(error(operation.key, Code::Busy));
        }
        let id = next_lease(&mut state, operation.key)?;
        state.data_mut()?.stages.push(super::super::stage::RetainedStage {
            id,
            native: None,
            object: plan.completion,
            reserved: 0,
            written: expected.bytes(),
            flushed: None,
            recovered: Some(expected),
            role: None,
            activated: false,
            recovery_predecessor: Some(DiagnosticPredecessor::Exact {
                file_identity: original.file_identity(),
                sha256: original.sha256(),
            }),
        });
        let index = state.data_mut()?.stages.len() - 1;
        let (native_id, actual) = state
            .call_native(|port| {
                port.recover_stage(plan.completion, plan.operation, expected, context)
            })?
            .map_err(|cause| native(operation.key, Code::Read, cause))?;
        state.data_mut()?.stages[index].native = Some(native_id);
        if native_id == 0 || !expected.matches(actual) {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        check(&mut state, operation.key, context, false)?;
        state.data_mut()?.stages[index].flushed = Some(actual);
        Ok(DiagnosticStage { shared: self.shared.clone(), operation: operation.key, id })
    }
}
