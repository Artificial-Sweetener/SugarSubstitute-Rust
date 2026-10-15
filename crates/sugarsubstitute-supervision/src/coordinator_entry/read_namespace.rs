// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Detach and restore the identical native namespace capsule outside provider locks.
use super::namespace::{NamespaceLifetime, NativeState};
use crate::{
    ClaimError,
    diagnostic_storage::{DiagnosticNativeGrant, ProtectedDiagnosticWriteRequest},
};
use std::sync::{Arc, atomic::Ordering};
use sugarsubstitute_contracts::execution::JobContext;
/// Retain actual source pins and data before its unchanged original lifetime guard.
pub(crate) struct NamespaceReadLoan {
    state: Option<NativeState>,
    lifetime: Arc<NamespaceLifetime>,
    writer: Option<WriterBinding>,
}
struct WriterBinding {
    original: DiagnosticNativeGrant,
    execution: sugarsubstitute_contracts::execution::ExecutionId,
    operation: sugarsubstitute_contracts::incident::LogicalStoreOperationKey,
}
fn rejected() -> ClaimError {
    ClaimError::diagnostic_allocation(
        crate::diagnostic_storage::DiagnosticNativeAllocationFailure::namespace_rejected(),
    )
}
impl NamespaceReadLoan {
    /// Borrow the same live namespace for one actually admitted existing-only reader.
    pub(crate) fn take(
        request: &crate::committed_reader::ProtectedCommittedReadRequest,
        original: &sugarsubstitute_execution::CommittedReaderOrigin,
    ) -> Result<Self, ClaimError> {
        let context = original.original_context().map_err(|_| rejected())?;
        if request.execution_owner() != context.owner_id() {
            return Err(rejected());
        }
        let lifetime = &request.lifetime;
        if !lifetime.open.load(Ordering::Acquire) {
            return Err(rejected());
        }
        let mut state = lifetime.state.lock().map_err(|_| rejected())?;
        if state.borrowed
            || state.retired
            || state.memory.is_none()
            || state
                .namespace
                .as_ref()
                .is_none_or(|namespace| namespace.namespace != request.namespace())
        {
            return Err(rejected());
        }
        let original_state = std::mem::replace(&mut *state, NativeState::new());
        state.borrowed = true;
        drop(state);
        Ok(Self { state: Some(original_state), lifetime: Arc::clone(lifetime), writer: None })
    }
    /// Validate the genuine current native grant before detaching the same source.
    /// Original controls execute outside the namespace mutex.
    pub(crate) fn take_writer(
        request: &ProtectedDiagnosticWriteRequest,
        original: &DiagnosticNativeGrant,
        current: &JobContext,
    ) -> Result<Self, ClaimError> {
        if request.execution() != current.owner_id() {
            return Err(rejected());
        }
        original.bind_namespace(request, current).map_err(ClaimError::diagnostic_allocation)?;
        let lifetime = &request.lifetime;
        if !lifetime.open.load(Ordering::Acquire) {
            return Err(rejected());
        }
        let mut state = lifetime.state.lock().map_err(|_| rejected())?;
        if state.borrowed
            || state.retired
            || state.memory.is_none()
            || state.namespace.as_ref().is_none_or(|namespace| {
                namespace.namespace != request.operation().owner().namespace
            })
        {
            return Err(rejected());
        }
        let original_state = std::mem::replace(&mut *state, NativeState::new());
        state.borrowed = true;
        drop(state);
        Ok(Self {
            state: Some(original_state),
            lifetime: Arc::clone(lifetime),
            writer: Some(WriterBinding {
                original: original.clone(),
                execution: request.execution(),
                operation: request.operation(),
            }),
        })
    }
    /// Validate the exact live writer source without granting Diagnostic effects on Protected handles.
    /// The original source allocation and armed diagnostic slot remain borrowed until this loan drops.
    pub(crate) fn validate_writer(
        &self,
        original: &DiagnosticNativeGrant,
        current: &JobContext,
    ) -> Result<&crate::protected_storage::account::ResolvedNamespace, ClaimError> {
        let binding = self.writer.as_ref().ok_or_else(rejected)?;
        if !original.matches_original(&binding.original) || binding.execution != current.owner_id()
        {
            return Err(rejected());
        }
        original
            .validate_namespace(&self.lifetime, binding.operation, current)
            .map_err(ClaimError::diagnostic_allocation)?;
        let state = self.state.as_ref().ok_or_else(rejected)?;
        let memory = state.memory.as_ref().ok_or_else(rejected)?;
        let resolved = self.resolved()?;
        if state.retired || resolved.namespace != binding.operation.owner().namespace {
            return Err(rejected());
        }
        let (_, slot, _) = resolved.diagnostic_directory();
        let pin = state.handles.get(slot).ok_or_else(rejected)?;
        if !pin.is_armed()
            || !matches!(pin.budget(), crate::native_claims::files::Budget::Protected(ref held) if Arc::ptr_eq(held, memory))
        {
            return Err(rejected());
        }
        Ok(resolved)
    }
    pub(crate) fn resolved(
        &self,
    ) -> Result<&crate::protected_storage::account::ResolvedNamespace, ClaimError> {
        self.state.as_ref().and_then(|state| state.namespace.as_ref()).ok_or_else(rejected)
    }
    pub(crate) fn handles(&self) -> Result<&[crate::native_claims::files::Handle], ClaimError> {
        self.state.as_ref().map(|state| state.handles.as_slice()).ok_or_else(rejected)
    }
}
impl Drop for NamespaceReadLoan {
    fn drop(&mut self) {
        let Some(state) = self.state.take() else { return };
        match self.lifetime.state.lock() {
            Ok(mut retained)
                if retained.borrowed
                    && retained.handles.is_empty()
                    && retained.memory.is_none()
                    && retained.namespace.is_none()
                    && retained.election.is_none()
                    && retained.recovery_coordinator.is_none()
                    && retained.record_origin.is_none() =>
            {
                *retained = state;
            }
            _ => {
                std::mem::forget(state);
                std::mem::forget(Arc::clone(&self.lifetime));
                tracing::error!(
                    operation = "diagnostic_namespace_retained",
                    "Native namespace restore custody retained"
                );
            }
        }
    }
}
