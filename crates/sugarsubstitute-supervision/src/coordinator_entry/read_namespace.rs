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
        Ok(Self { state: Some(original_state), lifetime: Arc::clone(lifetime) })
    }
    /// Validate the genuine current native grant before detaching the same source.
    /// Original controls execute outside the namespace mutex.
    pub(crate) fn take_writer(
        request: &ProtectedDiagnosticWriteRequest,
        original: &DiagnosticNativeGrant,
        current: &JobContext,
    ) -> Result<Self, ClaimError> {
        original.check(current).map_err(ClaimError::diagnostic_allocation)?;
        if request.execution() != current.owner_id() {
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
            || state.namespace.as_ref().is_none_or(|namespace| {
                namespace.namespace != request.operation().owner().namespace
            })
        {
            return Err(rejected());
        }
        let original_state = std::mem::replace(&mut *state, NativeState::new());
        state.borrowed = true;
        drop(state);
        Ok(Self { state: Some(original_state), lifetime: Arc::clone(lifetime) })
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
