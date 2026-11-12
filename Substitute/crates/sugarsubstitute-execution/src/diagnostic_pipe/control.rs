// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit typed diagnostic operations and drain fixed transport facts.
use super::*;
use crate::lifecycle_pipe::{Control, Observation, PeerBinding};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    execution::{JobCompletion, Moment, OwnedJob},
    incident::DiagnosticKey,
};
/// Own one persistent diagnostic native session until actual retirement.
pub struct DiagnosticPipeSession {
    pub(crate) control: DiagnosticPipeControl,
    pub(crate) completion: OwnedJob<DiagnosticPipeReport, LifecyclePipeFailure>,
}
impl DiagnosticPipeSession {
    /// Borrow the fixed command owner without exposing a scheduler or raw native handle.
    pub const fn control(&self) -> &DiagnosticPipeControl {
        &self.control
    }
    /// Drain at most min(requested,32) fixed observations, preserving frame custody.
    pub fn drain(&mut self, max: NonZeroUsize) -> DiagnosticPipeObservations {
        let mut result =
            DiagnosticPipeObservations { items: std::array::from_fn(|_| None), length: 0 };
        for slot in result.items.iter_mut().take(max.get().min(32)) {
            let Some(value) =
                self.control.inner.take_observation().and_then(Observation::diagnostic)
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
    ) -> Option<JobCompletion<DiagnosticPipeReport, LifecyclePipeFailure>> {
        self.completion.try_take_completion()
    }
}
/// Retain fixed drained facts and their owned charged frame guards.
pub struct DiagnosticPipeObservations {
    items: [Option<DiagnosticPipeObservation>; 32],
    length: usize,
}
impl DiagnosticPipeObservations {
    /// Return the actual drained count.
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Return whether no fact was available.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}
impl IntoIterator for DiagnosticPipeObservations {
    type Item = DiagnosticPipeObservation;
    type IntoIter = std::iter::Flatten<std::array::IntoIter<Option<DiagnosticPipeObservation>, 32>>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter().flatten()
    }
}
/// Submit closed transport operations under the retained session's original budgets.
pub struct DiagnosticPipeControl {
    pub(crate) inner: Control,
}
impl DiagnosticPipeControl {
    /// Return the original finite handshake in the local execution epoch.
    pub fn original_deadline(&self) -> Moment {
        self.inner.original_deadline()
    }
    /// Return the immutable validated diagnostic binding as facts only.
    pub fn prepared_key(&self) -> Option<DiagnosticKey> {
        self.inner.binding().and_then(PeerBinding::diagnostic)
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
