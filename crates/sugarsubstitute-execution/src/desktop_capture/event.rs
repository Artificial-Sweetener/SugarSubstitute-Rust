// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the actual completion Event/Wait Box beneath its original outside claim.
use super::resources::CaptureEventClaim;
use crate::{
    capture_bundle::{CaptureBundleOrigin, CaptureBundleRetirement},
    native::{NativeObservationError, Signal},
};
use std::sync::Arc;

/// External Event/Wait effects are faked at this boundary; Active remains the owner.
/// Concrete native implementation uses the existing sole Event/registered-Wait owner.
pub(crate) trait CompletionEventLease: Send {
    /// Prepare actual outputs only after their exact two-output claim is retained.
    fn prepare(&mut self) -> Result<(), NativeObservationError>;
    /// Borrow actual owned event through one synchronous native call.
    fn raw(&self) -> usize;
    /// Check/quiesce/close before each fresh effect under the original cutoff.
    fn close(&mut self, proof: &CaptureBundleRetirement) -> Result<(), NativeObservationError>;
    /// Observe adopted checked closure and exact original counter allocation only.
    fn settled_for(&self, original: &CaptureBundleOrigin, identity: usize) -> bool;
}

/// Construct only inert same-owner storage; creation/registration follows on Active.
pub(super) type EventFactory =
    fn(Arc<Signal>, CaptureEventClaim) -> Result<CompletionEvent, NativeObservationError>;

/// Box is destroyed before the SAME original claim/control outside the native owner.
/// No separate claim or capacity is created for this wrapper.
pub(crate) struct CompletionEvent {
    pub(super) value: Box<dyn CompletionEventLease>,
    pub(super) original: CaptureEventClaim,
    settlement: Option<usize>,
}
impl CompletionEvent {
    /// Retain the same supplied external fake boundary beneath its real Core claim.
    #[cfg(test)]
    pub(super) fn from_boundary(
        value: Box<dyn CompletionEventLease>,
        original: CaptureEventClaim,
    ) -> Self {
        Self { value, original, settlement: None }
    }

    /// Construct inert native storage after concrete full layout preflight.
    #[cfg(windows)]
    pub(super) fn native(
        signal: Arc<Signal>,
        original: CaptureEventClaim,
    ) -> Result<Self, NativeObservationError> {
        if Self::storage_bytes() > super::resources::EVENT_METADATA_BYTES as usize {
            return Err(NativeObservationError::new(
                "capture_event_storage",
                std::io::Error::other("Original event metadata capacity exceeded"),
            ));
        }
        let mut event = crate::native_pipe::event::Event::new(signal);
        event.attach(crate::native_pipe::accounting::OutputAccounting::Capture(original.clone()));
        Ok(Self { value: Box::new(event), original, settlement: None })
    }
    /// Include concrete Event/Handle/Wait boxes in the original4096 preflight.
    #[cfg(windows)]
    pub(super) const fn storage_bytes() -> usize {
        size_of::<Self>()
            + CaptureEventClaim::storage_bytes()
            + crate::native_pipe::event::Event::storage_bytes()
    }
    /// Invalidate cached closure before the actual external creation/registration call.
    pub(super) fn prepare(&mut self) -> Result<(), NativeObservationError> {
        self.settlement = None;
        self.value.prepare()
    }
    /// Adopt only actual successful checked closure after the external boundary returns.
    /// The trait observation runs here OUTSIDE Core/cleanup/stage locks. A panic,
    /// failed close or foreign output leaves the previous unqualified cache untouched.
    pub(super) fn close(
        &mut self,
        proof: &CaptureBundleRetirement,
    ) -> Result<(), NativeObservationError> {
        self.value.close(proof)?;
        let identity = self.original.identity();
        if !proof.matches_origin(self.original.original())
            || !self.value.settled_for(self.original.original(), identity)
        {
            return Err(NativeObservationError::new(
                "capture_event_settlement",
                std::io::Error::other("Actual original Event/Wait closure unproven"),
            ));
        }
        self.settlement = Some(identity);
        Ok(())
    }
    /// Adopted facts are interpreted against this wrapper's SAME original allocation.
    pub(crate) fn settled_for(&self, original: &CaptureBundleOrigin, identity: usize) -> bool {
        self.settlement == Some(identity)
            && identity != 0
            && self.original.identity() == identity
            && self.original.original().same_allocation(original)
    }
}
#[cfg(windows)]
impl CompletionEventLease for crate::native_pipe::event::Event {
    fn prepare(&mut self) -> Result<(), NativeObservationError> {
        crate::native_pipe::event::Event::prepare(self)
            .map_err(|cause| NativeObservationError::new("capture_event_prepare", cause))
    }
    fn raw(&self) -> usize {
        crate::native_pipe::event::Event::raw(self).0 as usize
    }
    fn close(&mut self, proof: &CaptureBundleRetirement) -> Result<(), NativeObservationError> {
        self.close_capture(proof)
            .map_err(|cause| NativeObservationError::new("capture_event_close", cause))
    }
    fn settled_for(&self, original: &CaptureBundleOrigin, identity: usize) -> bool {
        crate::native_pipe::event::Event::settled_for(self, original, identity)
    }
}
