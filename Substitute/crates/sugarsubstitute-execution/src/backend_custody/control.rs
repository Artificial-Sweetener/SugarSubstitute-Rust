// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit typed backend_custody operations and drain fixed transport facts.
use super::*;
use crate::lifecycle_pipe::{Control, Observation, PeerBinding};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey,
    execution::{JobCompletion, Moment, OwnedJob},
};
/// Own one persistent backend_custody native session until actual retirement.
pub struct BackendCustodyPipeSession {
    pub(crate) control: BackendCustodyPipeControl,
    pub(crate) completion: OwnedJob<BackendCustodyPipeReport, LifecyclePipeFailure>,
}
impl BackendCustodyPipeSession {
    /// Borrow the fixed command owner without exposing a scheduler or raw native handle.
    pub const fn control(&self) -> &BackendCustodyPipeControl {
        &self.control
    }
    /// Drain at most min(requested,32) fixed observations, preserving frame custody.
    pub fn drain(&mut self, max: NonZeroUsize) -> BackendCustodyPipeObservations {
        let mut result =
            BackendCustodyPipeObservations { items: std::array::from_fn(|_| None), length: 0 };
        for slot in result.items.iter_mut().take(max.get().min(32)) {
            let Some(value) =
                self.control.inner.take_observation().and_then(Observation::backend_custody)
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
    ) -> Option<JobCompletion<BackendCustodyPipeReport, LifecyclePipeFailure>> {
        self.completion.try_take_completion()
    }
}
/// Retain fixed drained facts and their owned charged frame guards.
pub struct BackendCustodyPipeObservations {
    items: [Option<BackendCustodyPipeObservation>; 32],
    length: usize,
}
impl BackendCustodyPipeObservations {
    /// Return the actual drained count.
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Return whether no fact was available.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}
impl IntoIterator for BackendCustodyPipeObservations {
    type Item = BackendCustodyPipeObservation;
    type IntoIter =
        std::iter::Flatten<std::array::IntoIter<Option<BackendCustodyPipeObservation>, 32>>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter().flatten()
    }
}
/// Submit closed transport operations under the retained session's original budgets.
pub struct BackendCustodyPipeControl {
    pub(crate) inner: Control,
}
impl BackendCustodyPipeControl {
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
    /// Return the immutable validated backend_custody binding as facts only.
    pub fn prepared_key(&self) -> Option<BackendCustodyKey> {
        self.inner.binding().and_then(PeerBinding::backend_custody)
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
    /// Transfer the exact charged guard; rejection retains its original bytes and custody.
    pub fn write(
        &self,
        operation: LifecyclePipeOperationId,
        original: Moment,
        buffer: LifecyclePipeWriteBuffer,
        length: usize,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        self.inner.write(operation, original, buffer, length)
    }
    /// Send only a canonical heartbeat on the established custody session.
    /// Its independent active-time watchdog bounds silence; OS sleep cannot expire this I/O.
    /// Cancellation still requires checked native retirement before releasing the buffer.
    /// # Errors
    /// Rejects foreign sessions, other payloads, reused operations and occupied storage.
    pub fn write_heartbeat(
        &self,
        operation: LifecyclePipeOperationId,
        buffer: LifecyclePipeWriteBuffer,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        self.inner.write_heartbeat(operation, buffer)
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
