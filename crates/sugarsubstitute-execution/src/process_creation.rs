// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one finite native process creation across source admission, acknowledgements and resume.
mod memory;
#[cfg(windows)]
pub(crate) use memory::SectionOutput;
mod recovery;
mod retirement;
pub(crate) mod source;
mod submission;
pub use recovery::{RecoveryCreationBeginRejection, RecoveryCreationPermit};
pub use retirement::ProcessCreationRetirement;
pub(crate) mod worker;
use crate::native::NativeObservationError;
pub use memory::ProcessCreationMemory;
use sugarsubstitute_contracts::execution::{JobContext, Moment};
pub use worker::{ProcessCreationFailure, ProcessCreationReport};

/// Original fixed family/control partition within the creation reservation.
pub const PROCESS_CREATION_STORAGE_BYTES: u64 = 131_072;

/// Notify only the admitted creation owner after its native/transport observation changes.
#[derive(Clone)]
pub struct ProcessCreationWake {
    signal: std::sync::Arc<crate::native::Signal>,
    _memory: ProcessCreationMemory,
}
impl ProcessCreationWake {
    /// Notify the native-thread actor after actual suspended-child identity publication.
    /// Call outside adapter locks after retaining every physical output. This coalesces
    /// through the existing application wake and creates no terminal observation.
    /// # Errors
    /// Rejects stopped/foreign/expired creation source before notification.
    pub fn application_observed(
        &self,
    ) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        self._memory.application_observed()
    }
    /// Wake the original owner without exposing a runtime or general scheduler port.
    pub fn observed(&self) {
        self.signal.complete();
    }
}

/// One finite progress observation; completion requires both acknowledgements and resume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessCreationProgress {
    /// The original transaction still owns its family or an outstanding response.
    Pending,
    /// The same suspended root was acknowledged and actually resumed.
    Started,
}

/// External native family owner retained by the existing execution native worker.
/// Construction is inert. The driver adopts every partial output before effects;
/// preparation/root acknowledgements remain tied to their actual original cells.
pub trait ProcessCreationDriver: Send {
    /// Install the original family and source before any kernel effect.
    /// # Errors
    /// Preserves partial output custody and the original failure.
    fn initialize(
        &mut self,
        context: &JobContext,
        memory: &ProcessCreationMemory,
        wake: ProcessCreationWake,
    ) -> Result<(), NativeObservationError>;
    /// Advance one bounded pass without waiting for another worker or renewing time.
    /// # Errors
    /// Retains exact native/protocol failures for the same creation transaction.
    fn progress(
        &mut self,
        context: &JobContext,
        now: Moment,
    ) -> Result<ProcessCreationProgress, NativeObservationError>;
    /// Retire temporary transaction custody or transfer the started family to its registry.
    /// A true result requires actual checked settlement/transfer of every output;
    /// transport closure alone cannot settle a remote duplicated handle.
    /// # Errors
    /// Failed retirement remains owned and preserves its first cause.
    fn retire(&mut self, context: &JobContext, now: Moment)
    -> Result<bool, NativeObservationError>;
}
