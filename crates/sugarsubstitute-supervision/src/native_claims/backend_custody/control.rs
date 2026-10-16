// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind namespace outputs to the original helper's single close-only transaction.
use crate::ClaimError;
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_execution::backend_custody::{BackendCustodyCleanup, BackendCustodyMemory};

/// All namespace/output aliases share the same first original cleanup proof.
pub(crate) struct WitnessControl {
    state: Option<Arc<Mutex<Option<BackendCustodyCleanup>>>>,
    memory: BackendCustodyMemory,
}
impl WitnessControl {
    /// Return the installation/account namespace from the actual original session.
    pub(crate) fn expected_namespace(
        &self,
    ) -> Option<sugarsubstitute_contracts::lifecycle::NamespaceId> {
        self.memory.key().map(|key| key.owner().namespace)
    }
    /// Construct only from the admitted worker's unchanged metadata guard.
    pub(crate) fn new(memory: &BackendCustodyMemory) -> Self {
        Self { state: Some(Arc::new(Mutex::new(None))), memory: memory.clone() }
    }
    /// Latch one first cleanup before touching already-owned namespace outputs.
    pub(crate) fn cleanup(
        &self,
        original_until: Moment,
    ) -> Result<BackendCustodyCleanup, ClaimError> {
        let proof = self
            .memory
            .begin_cleanup(original_until)
            .map_err(|cause| ClaimError::new("custody_namespace_cleanup", cause))?;
        let state =
            self.state.as_ref().ok_or_else(|| ClaimError::invalid("custody_namespace_closed"))?;
        let mut state = super::super::lock(state);
        if state.is_none() {
            *state = Some(proof.clone());
        }
        Ok(proof)
    }
    /// Observe the already-latched actual helper cleanup, without sampling or renewing it.
    pub(crate) fn cleanup_until(&self) -> Option<Moment> {
        let state = self.state.as_ref()?;
        super::super::lock(state).as_ref().map(BackendCustodyCleanup::until)
    }
    /// Observe providers outside the state lock and preserve actual first control.
    pub(crate) fn check(&self) -> Result<(), ClaimError> {
        let state =
            self.state.as_ref().ok_or_else(|| ClaimError::invalid("custody_namespace_closed"))?;
        let proof = super::super::lock(state).clone();
        match proof {
            Some(proof) => proof.check(),
            None => self.memory.check_registration().map(|_| ()),
        }
        .map_err(|cause| ClaimError::new("custody_namespace_control", cause))
    }
    /// Compare the same namespace control allocation without provider calls.
    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        match (&self.state, &other.state) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}
impl Clone for WitnessControl {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), memory: self.memory.clone() }
    }
}
impl Drop for WitnessControl {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            drop(Arc::into_inner(state));
        }
    }
}
