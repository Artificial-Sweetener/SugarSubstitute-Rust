// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the external custody owner to the existing admitted native worker.
use super::BackendCustodyMemory;
use crate::native::{NativeObservationError, Signal};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, Moment};

/// Wake only this admitted custody owner after its native observation changes.
/// This callback carries no runtime handle or generic scheduling port.
#[derive(Clone)]
pub struct BackendCustodyWake {
    pub(crate) signal: Arc<Signal>,
    pub(crate) job: sugarsubstitute_contracts::execution::JobId,
}
impl BackendCustodyWake {
    /// Publish a changed custody observation to its original execution owner.
    pub fn observed(&self) {
        self.signal.complete();
    }
}

/// One bounded progress result from the actual retained native custodian.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendCustodyProgress {
    /// Native callbacks, registrations or cleanup still retain custody.
    Pending,
    /// Every registered family and native output was actually settled.
    Settled,
}

/// Own all native outputs before each fallible kernel or provider call.
/// Construction is inert. Initialization and progress run on the existing native
/// worker outside execution locks. Parent-loss cleanup must latch one immutable
/// cutoff and retain uncertain outputs; availability of names/locks is not proof.
pub trait BackendCustodyDriver: Send {
    /// Install original observation callbacks after this driver is durably owned.
    /// # Errors
    /// Retains every partial output and its exact first cause for retirement.
    fn initialize(
        &mut self,
        context: &JobContext,
        metadata: &BackendCustodyMemory,
        wake: BackendCustodyWake,
    ) -> Result<(), NativeObservationError>;
    /// Perform one bounded pass using the original job and observed sample.
    /// # Errors
    /// Preserves actual kernel/protocol failure without manufacturing settlement.
    fn progress(
        &mut self,
        context: &JobContext,
        now: Moment,
    ) -> Result<BackendCustodyProgress, NativeObservationError>;
    /// Stop new registration and progress the original owned cleanup transaction.
    /// Success means all callbacks, registrations, locks, handles and family
    /// storage have actually retired. A failed close remains owned without retry
    /// that changes the original cleanup cutoff.
    /// # Errors
    /// Retains original cleanup failure and all unresolved native outputs.
    fn retire(&mut self, context: &JobContext, now: Moment)
    -> Result<bool, NativeObservationError>;
}
