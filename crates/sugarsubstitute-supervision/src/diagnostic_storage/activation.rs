// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish private actual activation observations after checked native settlement.
use super::*;

/// Observe exact durable active bytes after checked flush/readback and handle closure.
/// A present-state reconciliation does not claim a historical activation time.
pub struct DiagnosticActivation {
    pub(super) shared: Shared,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) object: DiagnosticObject,
    pub(super) facts: DiagnosticReadFacts,
}
impl DiagnosticActivation {
    /// Observe the exact typed active object.
    pub const fn object(&self) -> DiagnosticObject {
        self.object
    }
    /// Observe actual active file identity, digest and length.
    pub const fn facts(&self) -> DiagnosticReadFacts {
        self.facts
    }
}
impl DiagnosticStorage {
    /// Activate exact flushed bytes under writer exclusion and predecessor comparison.
    /// # Errors
    /// Uncertain rename/flush/close preserves the retained stage; no proof is returned.
    pub fn activate(
        &mut self,
        stage: &mut DiagnosticStage,
        expected: DiagnosticPredecessor,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticActivation, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        stage::mutation(&mut state, operation.key)?;
        if !Shared::ptr_eq(&self.shared, &stage.shared) || stage.operation != operation.key {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let index = state
            .data_mut()?
            .stages
            .iter()
            .position(|value| value.id == stage.id)
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        let retained = &state.data_mut()?.stages[index];
        if retained.recovery_predecessor.is_some_and(|original| original != expected) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let flushed = retained.flushed.ok_or_else(|| error(operation.key, Code::Flush))?;
        let object = retained.object;
        let id = retained.native.ok_or_else(|| error(operation.key, Code::Closed))?;
        let facts = state
            .call_native(|port| port.activate(id, object, expected, context))?
            .map_err(|cause| native(operation.key, Code::ActivationUncertain, cause))?;
        if facts != flushed {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        check(&mut state, operation.key, context, false)?;
        state.data_mut()?.stages.remove(index);
        state.data_mut()?.inventory_complete = false;
        Ok(DiagnosticActivation {
            shared: self.shared.clone(),
            operation: operation.key,
            object,
            facts,
        })
    }
    /// Observe exact current durable active bytes after actual flush/readback/closure.
    /// # Errors
    /// Rejects absence, unchecked predecessor or changed actual target under exclusion.
    pub fn reconcile_activation(
        &mut self,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticActivation, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        stage::mutation(&mut state, operation.key)?;
        let DiagnosticPredecessor::Exact { file_identity, sha256 } = expected else {
            return Err(error(operation.key, Code::InvalidIdentity));
        };
        let namespace = state.data_mut()?.namespace;
        if object.namespace().is_some_and(|value| Some(value) != namespace) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let facts = state
            .call_native(|port| port.reconcile_activation(object, expected, context))?
            .map_err(|cause| native(operation.key, Code::ActivationUncertain, cause))?;
        if facts.identity != file_identity || facts.sha256 != sha256 || facts.bytes > object.bound()
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        check(&mut state, operation.key, context, false)?;
        Ok(DiagnosticActivation {
            shared: self.shared.clone(),
            operation: operation.key,
            object,
            facts,
        })
    }
}
