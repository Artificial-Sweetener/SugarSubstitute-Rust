// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one finite managed-creation job alive across genuine custody acknowledgements.
mod memory;
mod retirement;
mod submission;
pub use retirement::CustodiedProcessRetirement;
pub(crate) mod worker;
use crate::native::NativeObservationError;
pub use memory::CustodiedProcessMemory;
use sugarsubstitute_contracts::execution::{JobContext, Moment};
pub use worker::{CustodiedProcessFailure, CustodiedProcessReport};

/// Original fixed family/control partition within the creation reservation.
pub const CUSTODIED_PROCESS_STORAGE_BYTES: u64 = 131_072;

/// Notify only the admitted creation owner after its native/transport observation changes.
#[derive(Clone)]
pub struct CustodiedProcessWake {
    signal: std::sync::Arc<crate::native::Signal>,
    _memory: CustodiedProcessMemory,
}
impl CustodiedProcessWake {
    /// Wake the original owner without exposing a runtime or general scheduler port.
    pub fn observed(&self) {
        self.signal.complete();
    }
}

/// One finite progress observation; completion requires both acknowledgements and resume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodiedProcessProgress {
    /// The original transaction still owns its family or an outstanding response.
    Pending,
    /// The same suspended root was acknowledged and actually resumed.
    Started,
}

/// External native family owner retained by the existing execution native worker.
/// Construction is inert. The driver adopts every partial output before effects;
/// preparation/root acknowledgements remain tied to their actual original cells.
pub trait CustodiedProcessDriver: Send {
    /// Install the original family and source before any kernel effect.
    /// # Errors
    /// Preserves partial output custody and the original failure.
    fn initialize(
        &mut self,
        context: &JobContext,
        memory: &CustodiedProcessMemory,
        wake: CustodiedProcessWake,
    ) -> Result<(), NativeObservationError>;
    /// Advance one bounded pass without waiting for another worker or renewing time.
    /// # Errors
    /// Retains exact native/protocol failures for the same creation transaction.
    fn progress(
        &mut self,
        context: &JobContext,
        now: Moment,
    ) -> Result<CustodiedProcessProgress, NativeObservationError>;
    /// Retire temporary transaction custody or transfer the started family to its registry.
    /// A true result requires actual checked settlement/transfer of every output;
    /// transport closure alone cannot settle a remote duplicated handle.
    /// # Errors
    /// Failed retirement remains owned and preserves its first cause.
    fn retire(&mut self, context: &JobContext, now: Moment)
    -> Result<bool, NativeObservationError>;
}
