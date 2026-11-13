// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deliver checked copied units together with the original whole preparation owner.

use super::{
    cause::EntryEnvironmentCause,
    owner::EntryPreparationOwner,
    state::{EntryEnvironmentPhase, NativeCapture},
};
use std::{error::Error, fmt};

/// Inert facts from the actual capture, without native release or process authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryEnvironmentObservation {
    /// Actual original owner phase when the observation was taken.
    pub phase: EntryEnvironmentPhase,
    /// Number of original UTF16 units inspected under the copy policy.
    pub inspected_units: usize,
    /// Retained later causes, bounded to eight.
    pub secondary_count: usize,
    /// Additional observed causes omitted from that bounded inventory.
    pub omitted_causes: u64,
}
impl EntryEnvironmentObservation {
    pub(super) fn from_capture(capture: &NativeCapture) -> Self {
        Self {
            phase: capture.phase(),
            inspected_units: capture.inspected,
            secondary_count: capture.secondary.iter().filter(|cause| cause.is_some()).count(),
            omitted_causes: capture.omitted,
        }
    }
}

/// Keep every genuine failure cause before its unchanged original whole guard.
pub struct EntryEnvironmentFailure {
    cause: Option<EntryEnvironmentCause>,
    observation: Option<EntryEnvironmentObservation>,
    _owner: EntryPreparationOwner,
}
pub(super) struct NativeFailure {
    pub(super) cause: EntryEnvironmentCause,
    pub(super) secondary: [Option<EntryEnvironmentCause>; 8],
}
impl EntryEnvironmentFailure {
    pub(super) fn from_shape(
        source: sugarsubstitute_contracts::coordinator_entry::EntryShapeError,
        owner: EntryPreparationOwner,
    ) -> Self {
        let observation = owner.capture_observation();
        Self { cause: Some(EntryEnvironmentCause::Shape { source }), observation, _owner: owner }
    }
    #[cfg(any(windows, test))]
    pub(super) fn from_capture(capture: &mut NativeCapture, owner: EntryPreparationOwner) -> Self {
        let observation = Some(EntryEnvironmentObservation::from_capture(capture));
        owner.close_publication();
        let fault = NativeFailure {
            cause: capture.first.take().unwrap_or(EntryEnvironmentCause::InvalidNativeState),
            secondary: std::mem::replace(&mut capture.secondary, std::array::from_fn(|_| None)),
        };
        if let Ok(payload) = owner.payload()
            && let Err(rejected) = payload.native_failure.set(fault)
        {
            drop(rejected);
        }
        Self { cause: None, observation, _owner: owner }
    }
    pub(super) fn from_control(
        cause: super::EntryPreparationInputError,
        owner: EntryPreparationOwner,
    ) -> Self {
        let observation = owner.capture_observation();
        Self {
            cause: Some(EntryEnvironmentCause::Preparation { source: cause }),
            observation,
            _owner: owner,
        }
    }
    /// Borrow the first actual cause without transferring its retaining owner.
    pub fn cause(&self) -> Option<&EntryEnvironmentCause> {
        self.cause
            .as_ref()
            .or_else(|| self._owner.payload().ok()?.native_failure.get().map(|fault| &fault.cause))
    }
    /// Iterate genuine later causes in observation order.
    pub fn secondary(&self) -> impl Iterator<Item = &EntryEnvironmentCause> {
        self._owner
            .payload()
            .ok()
            .and_then(|payload| payload.native_failure.get())
            .into_iter()
            .flat_map(|fault| fault.secondary.iter())
            .filter_map(Option::as_ref)
    }
    /// Observe categorical copy/retirement facts, granting no native capability.
    pub const fn observation(&self) -> Option<EntryEnvironmentObservation> {
        self.observation
    }
}
impl fmt::Debug for EntryEnvironmentFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryEnvironmentFailure")
            .field("observation", &self.observation)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for EntryEnvironmentFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original environment capture failed")
    }
}
impl Error for EntryEnvironmentFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause().map(|cause| cause as &(dyn Error + 'static))
    }
}

/// Preserve original copied order and UTF16, with inseparable full-claim custody.
/// Construction requires actual checked native release and original controls.
pub struct CapturedEntryEnvironment {
    pub(super) units: Vec<u16>,
    pub(super) owner: EntryPreparationOwner,
}
impl CapturedEntryEnvironment {
    /// Borrow exact original units, including their original double terminator.
    pub fn units(&self) -> &[u16] {
        &self.units
    }
}
impl fmt::Debug for CapturedEntryEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedEntryEnvironment { retained, redacted }")
    }
}
