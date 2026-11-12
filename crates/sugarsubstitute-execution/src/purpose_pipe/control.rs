// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit purpose-typed operations under their direction rule and drain fixed transport facts.
use super::*;
use crate::lifecycle_pipe::{Control, Observation, PeerBinding};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    execution::{JobCompletion, Moment, OwnedJob},
    update::attempt::codec,
};
/// Own one persistent purpose-typed native session until actual retirement.
pub struct PurposePipeSession {
    pub(crate) control: PurposePipeControl,
    pub(crate) completion: OwnedJob<PurposePipeReport, LifecyclePipeFailure>,
}
impl PurposePipeSession {
    /// Borrow the fixed command owner without exposing a scheduler or raw native handle.
    pub const fn control(&self) -> &PurposePipeControl {
        &self.control
    }
    /// Drain at most min(requested,32) fixed observations, preserving frame custody.
    pub fn drain(&mut self, max: NonZeroUsize) -> PurposePipeObservations {
        let mut result =
            PurposePipeObservations { items: std::array::from_fn(|_| None), length: 0 };
        for slot in result.items.iter_mut().take(max.get().min(32)) {
            let Some(value) = self.control.inner.take_observation().and_then(Observation::purpose)
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
    ) -> Option<JobCompletion<PurposePipeReport, LifecyclePipeFailure>> {
        self.completion.try_take_completion()
    }
}
/// Retain fixed drained facts and their owned charged frame guards.
pub struct PurposePipeObservations {
    items: [Option<PurposePipeObservation>; 32],
    length: usize,
}
impl PurposePipeObservations {
    /// Return the actual drained count.
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Return whether no fact was available.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}
impl IntoIterator for PurposePipeObservations {
    type Item = PurposePipeObservation;
    type IntoIter = std::iter::Flatten<std::array::IntoIter<Option<PurposePipeObservation>, 32>>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter().flatten()
    }
}
/// Submit closed transport operations under the retained session's original budgets and the
/// purpose's direction rule.
pub struct PurposePipeControl {
    pub(crate) inner: Control,
    pub(crate) purpose: Purpose,
    pub(crate) side: Side,
}
impl PurposePipeControl {
    /// Return the original finite handshake in the local execution epoch.
    pub fn original_deadline(&self) -> Moment {
        self.inner.original_deadline()
    }
    /// Return the purpose fixed at reservation.
    pub const fn purpose(&self) -> Purpose {
        self.purpose
    }
    /// Return the immutable validated binding as facts only.
    pub fn prepared_key(&self) -> Option<PurposeKey> {
        self.inner.binding().and_then(PeerBinding::purpose)
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
    /// The parent of a helper-reporting purpose is rejected with `LifecyclePipeError::Direction`;
    /// it sends only `write_cancel_request`.
    pub fn write(
        &self,
        operation: LifecyclePipeOperationId,
        original: Moment,
        buffer: LifecyclePipeWriteBuffer,
        length: usize,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        if self.side == Side::Parent && self.purpose.direction() == Direction::HelperReports {
            return Err(LifecyclePipeWriteRejection {
                error: LifecyclePipeError::Direction,
                buffer,
            });
        }
        self.inner.write(operation, original, buffer, length)
    }
    /// Send an update attempt's one parent frame, a cancel request. This owner encodes it for
    /// the bound attempt, so the parent can never write anything else.
    /// Other purposes and the helper side are rejected with `Direction`, an unbound session with
    /// `NotReady`; rejection retains the original buffer.
    pub fn write_cancel_request(
        &self,
        operation: LifecyclePipeOperationId,
        original: Moment,
        mut buffer: LifecyclePipeWriteBuffer,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        if self.side != Side::Parent || self.purpose != Purpose::UpdateAttempt {
            return Err(LifecyclePipeWriteRejection {
                error: LifecyclePipeError::Direction,
                buffer,
            });
        }
        let Some(channel) = self.prepared_key().and_then(PurposeKey::update_attempt) else {
            return Err(LifecyclePipeWriteRejection {
                error: LifecyclePipeError::NotReady,
                buffer,
            });
        };
        let Ok(bytes) = codec::encode_cancel(channel.attempt()) else {
            return Err(LifecyclePipeWriteRejection {
                error: LifecyclePipeError::InvalidMessage,
                buffer,
            });
        };
        let Some(target) = buffer.as_mut_bytes().get_mut(..bytes.len()) else {
            return Err(LifecyclePipeWriteRejection {
                error: LifecyclePipeError::InvalidMessage,
                buffer,
            });
        };
        target.copy_from_slice(&bytes);
        self.inner.write(operation, original, buffer, bytes.len())
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
