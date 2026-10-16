// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit typed recovery_supervisor operations and drain fixed transport facts.
use super::*;
use crate::lifecycle_pipe::{Control, Observation, PeerBinding};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    execution::{JobCompletion, Moment, OwnedJob},
    recovery_supervisor::RecoveryGrantKey,
};
/// Own one persistent recovery_supervisor native session until actual retirement.
pub struct RecoveryPipeSession {
    pub(crate) control: RecoveryPipeControl,
    pub(crate) completion: OwnedJob<RecoveryPipeReport, LifecyclePipeFailure>,
}
impl RecoveryPipeSession {
    /// Return this actual admitted job's original scope correlation.
    pub fn scope_id(&self) -> sugarsubstitute_contracts::execution::ScopeId {
        self.completion.scope_id()
    }
    /// Return this actual admitted job's unchanged complete specification.
    pub fn spec(&self) -> sugarsubstitute_contracts::execution::JobSpec {
        self.completion.spec()
    }
    /// Borrow the fixed command owner without exposing a scheduler or raw native handle.
    pub const fn control(&self) -> &RecoveryPipeControl {
        &self.control
    }
    /// Drain at most min(requested,32) fixed observations, preserving frame recovery.
    pub fn drain(&mut self, max: NonZeroUsize) -> RecoveryPipeObservations {
        let mut result =
            RecoveryPipeObservations { items: std::array::from_fn(|_| None), length: 0 };
        for slot in result.items.iter_mut().take(max.get().min(32)) {
            let Some(value) = self.control.inner.take_observation().and_then(Observation::recovery)
            else {
                break;
            };
            *slot = Some(value);
            result.length += 1;
        }
        result
    }
    /// Take the one typed outcome after real I/O, callbacks, handles and loans settle.
    pub fn take_terminal(
        &mut self,
    ) -> Option<JobCompletion<RecoveryPipeReport, LifecyclePipeFailure>> {
        self.completion.try_take_completion()
    }
}
/// Retain fixed drained facts and their owned charged frame guards.
pub struct RecoveryPipeObservations {
    items: [Option<RecoveryPipeObservation>; 32],
    length: usize,
}
impl RecoveryPipeObservations {
    /// Return the actual drained count.
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Return whether no fact was available.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}
impl IntoIterator for RecoveryPipeObservations {
    type Item = RecoveryPipeObservation;
    type IntoIter = std::iter::Flatten<std::array::IntoIter<Option<RecoveryPipeObservation>, 32>>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter().flatten()
    }
}
/// Submit closed transport operations under the retained session's original budgets.
pub struct RecoveryPipeControl {
    pub(crate) inner: Control,
}
impl RecoveryPipeControl {
    /// Transfer genuine checked native pipe settlement once, independently of its outcome.
    pub fn take_checked_retirement(&self) -> Option<RecoveryPipeRetirement> {
        let key = self.prepared_key()?;
        self.inner.take_checked_retirement().map(|proof| RecoveryPipeRetirement { proof, key })
    }
    /// Check actual running session identity and every current scope before effects.
    /// Historical peer facts cannot keep a stopped or closed session eligible.
    /// # Errors
    /// Preserves the original provider/admission cause after releasing all fences.
    pub fn validate_current_admission(&self) -> Result<(), crate::native::NativeObservationError> {
        crate::lifecycle_pipe::validate_current(&self.inner)
    }
    /// Observe the actual connection-local operation counter for an owned handoff.
    /// This matching fact grants no write/read or native authority.
    pub fn last_operation(&self) -> u64 {
        crate::scope::lock(&self.inner.shared.state).latest
    }
    /// Return the original finite handshake in the local execution epoch.
    pub fn original_deadline(&self) -> Moment {
        self.inner.original_deadline()
    }
    /// Return the immutable validated recovery_supervisor binding as facts only.
    pub fn prepared_key(&self) -> Option<RecoveryGrantKey> {
        self.inner.binding().and_then(PeerBinding::recovery)
    }
    /// Borrow the one charged output allocation; occupancy returns None.
    pub fn take_write_buffer(
        &self,
    ) -> Result<Option<LifecyclePipeWriteBuffer>, LifecyclePipeError> {
        self.inner.take_write_buffer()
    }
    /// Admit one finite receive without changing its original deadline.
    pub fn read(
        &self,
        operation: LifecyclePipeOperationId,
        original: Moment,
    ) -> Result<(), LifecyclePipeError> {
        self.inner.read(operation, original)
    }
    /// Admit an authenticated passive receive without an idle timer.
    pub fn read_passive(
        &self,
        operation: LifecyclePipeOperationId,
    ) -> Result<(), LifecyclePipeError> {
        self.inner.read_passive(operation)
    }
    /// Transfer the exact charged guard; rejection retains its original bytes and recovery.
    pub fn write(
        &self,
        operation: LifecyclePipeOperationId,
        original: Moment,
        buffer: LifecyclePipeWriteBuffer,
        length: usize,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        self.inner.write(operation, original, buffer, length)
    }
    /// Authenticate the immutable capability against actual kernel peer facts.
    pub fn verify_capability(
        &self,
        candidate: [u8; 32],
        original: Moment,
    ) -> Result<(), LifecyclePipeError> {
        self.inner.verify_capability(candidate, original)
    }
    /// Transfer the private child Hello capability once after bootstrap validation.
    pub fn take_client_capability(
        &self,
    ) -> Result<Option<LifecycleClientCapability>, LifecyclePipeError> {
        self.inner.take_client_capability()
    }
    /// Fence new issue and request actual cancellation without creating a cleanup budget.
    pub fn close(&self) {
        self.inner.close()
    }
}

/// Checked retirement of the exact admitted recovery transport, retaining original custody.
/// Neither terminal failure nor cancellation alone can construct this proof.
pub struct RecoveryPipeRetirement {
    proof: crate::lifecycle_pipe::LifecyclePipeRetirement,
    key: RecoveryGrantKey,
}
impl RecoveryPipeRetirement {
    /// Match actual allocation identity and the exact original recovery grant correlation.
    pub fn matches(&self, control: &RecoveryPipeControl) -> bool {
        self.proof.matches_recovery(control) && control.prepared_key() == Some(self.key)
    }
    /// Return the unchanged genuine grant correlation; this grants no source authority.
    pub const fn key(&self) -> RecoveryGrantKey {
        self.key
    }
    /// Return the actual execution owner identity.
    pub const fn execution_id(&self) -> sugarsubstitute_contracts::execution::ExecutionId {
        self.proof.execution_id()
    }
    /// Return the actual original admitted scope.
    pub const fn scope_id(&self) -> sugarsubstitute_contracts::execution::ScopeId {
        self.proof.scope_id()
    }
    /// Return the unchanged actual native job specification.
    pub const fn spec(&self) -> sugarsubstitute_contracts::execution::JobSpec {
        self.proof.spec()
    }
}
