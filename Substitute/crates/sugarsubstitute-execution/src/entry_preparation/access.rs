// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return the identical native allocation to its owner before publishing quiescence.

use super::owner::EntryPreparationOwner;
#[cfg(any(windows, test))]
use super::{
    EntryPreparationInputError,
    port::EnvironmentPort,
    slot::PreparationPhase,
    state::{EntryEnvironmentPhase, NativeCapture},
};
use crate::scope::lock;

impl EntryPreparationOwner {
    pub(super) fn capture_observation(&self) -> Option<super::EntryEnvironmentObservation> {
        let payload = self.payload().ok()?;
        lock(&payload.native)
            .capture
            .as_deref()
            .map(super::EntryEnvironmentObservation::from_capture)
    }
    #[cfg(any(windows, test))]
    pub(super) fn install_capture(
        &self,
        port: Box<dyn EnvironmentPort>,
    ) -> Result<(), EntryPreparationInputError> {
        self.check_native_layout(size_of_val(port.as_ref()))?;
        let payload = self.payload()?;
        let mut native = lock(&payload.native);
        if !native.acquisition_open || native.borrowed || native.capture.is_some() {
            return Err(EntryPreparationInputError::Closed);
        }
        native.acquisition_open = false;
        native.capture = Some(Box::new(NativeCapture::new(port)));
        Ok(())
    }
    #[cfg(any(windows, test))]
    pub(super) fn check_native_layout(
        &self,
        port_bytes: usize,
    ) -> Result<(), EntryPreparationInputError> {
        let actual = u64::try_from(port_bytes)
            .ok()
            .and_then(|port_bytes| super::layout::retained_native_bound()?.checked_add(port_bytes));
        let claim = self.claimed_bytes()?;
        if actual.is_none_or(|actual| actual > claim) {
            return Err(EntryPreparationInputError::Closed);
        }
        Ok(())
    }

    /// Mark access nonquiescent before moving the identical charged Box outside locks.
    #[cfg(any(windows, test))]
    pub(super) fn native_access(&self) -> Result<NativeAccess, EntryPreparationInputError> {
        let payload = self.payload()?;
        let capture = {
            let mut native = lock(&payload.native);
            if native.borrowed {
                return Err(EntryPreparationInputError::Closed);
            }
            let capture = native.capture.take().ok_or(EntryPreparationInputError::Closed)?;
            native.borrowed = true;
            capture
        };
        payload.publish_quiescence();
        Ok(NativeAccess { capture: Some(capture), owner: self.clone() })
    }

    /// Attempt only the original allocation's first Free on the existing native worker.
    /// Actual cancellation/expiry remains observable and cannot renew publication.
    #[cfg(any(windows, test))]
    pub(crate) fn progress_retirement(&mut self) {
        self.close_acquisition();
        let Ok(payload) = self.payload() else { return };
        if lock(&payload.native).callbacks != 0 {
            return;
        }
        let Ok(mut access) = self.native_access() else { return };
        let observed = self.clone();
        if let Some(capture) = access.capture_mut() {
            capture.retire(&mut || observed.native_control());
        }
    }

    /// Require returned callbacks and a checked release (or no issued Get).
    #[cfg(any(windows, test))]
    pub(crate) fn retirement_settled(&self) -> bool {
        let Ok(payload) = self.payload() else { return false };
        let native = lock(&payload.native);
        !native.borrowed
            && native.callbacks == 0
            && native.capture.as_ref().is_none_or(|capture| capture.native_settled)
    }

    #[cfg(any(windows, test))]
    pub(super) fn native_control(&self) -> Result<(), super::EntryPreparationControlCause> {
        self.observe_control().map(|_| ()).map_err(|cause| match cause {
            EntryPreparationInputError::Control(cause) => cause,
            EntryPreparationInputError::Unexpected(cause) => {
                super::EntryPreparationControlCause::Unexpected(cause)
            }
            cause => super::EntryPreparationControlCause::Boundary(Box::new(cause)),
        })
    }
}

/// Keep the native Box before its final whole guard on every normal/unwind path.
#[cfg(any(windows, test))]
pub(super) struct NativeAccess {
    capture: Option<Box<NativeCapture>>,
    owner: EntryPreparationOwner,
}
#[cfg(any(windows, test))]
impl NativeAccess {
    #[cfg(any(windows, test))]
    pub(super) fn capture_mut(&mut self) -> Option<&mut NativeCapture> {
        self.capture.as_deref_mut()
    }
}
#[cfg(any(windows, test))]
impl Drop for NativeAccess {
    fn drop(&mut self) {
        let Some(capture) = self.capture.take() else { return };
        let phase = if capture.native_settled {
            match capture.phase() {
                EntryEnvironmentPhase::Delivered => PreparationPhase::Delivered,
                EntryEnvironmentPhase::Prepared => PreparationPhase::Prepared,
                _ => PreparationPhase::Released,
            }
        } else {
            match capture.phase() {
                EntryEnvironmentPhase::GetterInFlight => PreparationPhase::GetterInFlight,
                EntryEnvironmentPhase::CopyInFlight => PreparationPhase::CopyInFlight,
                EntryEnvironmentPhase::FreeInFlight => PreparationPhase::FreeInFlight,
                EntryEnvironmentPhase::RetirementUncertain => PreparationPhase::Uncertain,
                _ => PreparationPhase::Held,
            }
        };
        if let Ok(payload) = self.owner.payload() {
            {
                let mut native = lock(&payload.native);
                native.capture = Some(capture);
                native.borrowed = false;
            }
            self.owner.publish_phase(phase);
            payload.publish_quiescence();
        }
    }
}
