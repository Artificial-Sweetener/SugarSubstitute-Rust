// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual native owners separately from cancellable unit-result jobs.
use crate::{
    CaptureStartInput, CrashAdmissionError, CrashError,
    native::{CaptureLease, NativePort},
};
use std::sync::{Arc, Mutex, MutexGuard};
use sugarsubstitute_contracts::{
    crash::{CaptureBudget, CaptureHandlerIdentity, CaptureObservation, CaptureState},
    execution::{CompletionBatch, Moment, OwnedJob},
    lifecycle::CoordinatorKey,
};
use sugarsubstitute_execution::windows::{NativeObservationError, ProcessExit, ProcessWatchPermit};

mod activity;
mod attach;
pub use attach::CaptureBootstrapRejection;
mod submission;
#[cfg(test)]
mod tests;
mod work;

type WatchJob = OwnedJob<ProcessExit, NativeObservationError>;
type WorkJob = OwnedJob<(), CrashError>;
pub(crate) fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    match value.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) struct Shared {
    pub(crate) native: Mutex<NativeState>,
    pub(crate) attempts: Mutex<Option<Box<crate::capture_endpoint::Attempts>>>,
    pub(crate) owner: CoordinatorKey,
    pub(crate) build: [u8; 64],
    pub(crate) build_length: usize,
    pub(crate) execution_id: sugarsubstitute_contracts::execution::ExecutionId,
    input: Mutex<Option<CaptureStartInput>>,
    permit: Mutex<Option<ProcessWatchPermit>>,
    port: Arc<dyn NativePort>,
    retained_memory: Mutex<Option<sugarsubstitute_contracts::execution::MemoryLease>>,
}
pub(crate) struct NativeState {
    pub(crate) lease: Option<Box<dyn CaptureLease>>,
    watch: Option<WatchJob>,
    pub(crate) registered: bool,
    pub(crate) handler: Option<CaptureHandlerIdentity>,
    pub(crate) retired: bool,
    deadline: Option<Moment>,
    close_deadline: Option<Moment>,
    errors: [Option<CrashError>; 9],
    omitted: u64,
}
impl NativeState {
    fn retain(&mut self, cause: CrashError) {
        if self.errors.iter().flatten().any(|old| old.same_cause(&cause)) {
            return;
        }
        if let Some(slot) = self.errors.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}

/// Immutable cached facts; reading this value never waits on native worker locks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureSnapshot {
    /// Supervisor lifetime being observed.
    pub owner: CoordinatorKey,
    /// Logical capture state, distinct from desktop/resource settlement.
    pub state: CaptureState,
    /// Kernel identity once actual retained outputs were observed.
    pub handler: Option<CaptureHandlerIdentity>,
    /// Actual exit code once a matching typed native watch completed successfully.
    pub handler_exit: Option<u32>,
    /// Fixed callback/module storage deliberately pinned until process exit.
    pub callback_pin_bytes: u64,
    /// Full original execution reservation still charged by the callback pin.
    /// A 64KiB allocation claim keeps its entire admitted lease charged.
    pub callback_pin_reservation_bytes: u64,
}

/// At most four categorical observations from one nonblocking owner turn.
#[derive(Debug)]
pub struct CaptureUpdates {
    values: [Option<CaptureObservation>; 4],
    length: usize,
    omitted: u64,
}
impl CaptureUpdates {
    fn new() -> Self {
        Self { values: [None; 4], length: 0, omitted: 0 }
    }
    fn push(&mut self, observation: CaptureObservation) {
        // A turn consumes at most start, close, watch, and final-retirement facts.
        if let Some(slot) = self.values.get_mut(self.length) {
            *slot = Some(observation);
            self.length += 1;
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    /// Borrow fixed bounded categorical values without native ownership.
    pub fn observations(&self) -> impl Iterator<Item = &CaptureObservation> {
        self.values[..self.length].iter().flatten()
    }
    /// Report categorical facts omitted after the fixed turn bound.
    pub const fn omitted_observation_count(&self) -> u64 {
        self.omitted
    }
}

struct RecoveryFailure {
    reason: CrashAdmissionError,
    _retained: Arc<Shared>,
}
impl std::fmt::Debug for RecoveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CaptureRecoveryFailure(..)")
    }
}
impl std::fmt::Display for RecoveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Capture rejection recovery failed")
    }
}
impl std::error::Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.reason)
    }
}

/// Rejected start preserves its still-owned native reservation and bounded input.
/// Callers may retry admission or explicitly consume `watch.cancel_unbound()`.
pub struct StartRejected {
    /// Actual admission reason; no bridge effect has occurred.
    pub reason: CrashAdmissionError,
    pending: Arc<Shared>,
}
impl StartRejected {
    /// Recover exact original admission reason, reservation and input without cloning.
    ///
    /// # Errors
    /// An unexpected execution-contract violation preserves its original admission
    /// cause and any still-retained partial input in this fixed shared owner.
    pub fn into_parts(
        self,
    ) -> Result<(CrashAdmissionError, ProcessWatchPermit, CaptureStartInput), CrashError> {
        let input = lock(&self.pending.input).take();
        let watch = lock(&self.pending.permit).take();
        match (input, watch) {
            (Some(input), Some(watch)) => Ok((self.reason, watch, input)),
            (input, watch) => {
                *lock(&self.pending.input) = input;
                *lock(&self.pending.permit) = watch;
                Err(CrashError::new(
                    sugarsubstitute_contracts::crash::CaptureOperation::Registration,
                    RecoveryFailure { reason: self.reason, _retained: self.pending },
                ))
            }
        }
    }
}

/// Single supervisor capture owner, independent of desktop/backend Job families.
///
/// Use an ordinary execution scope for finite work and a pre-reserved independent
/// native watch. Start/close effects and partial handles live outside job results.
/// Dropping this owner does not kill the handler or certify capture/settlement.
/// Explicit shutdown observes typed outcomes and checked native retirement first.
pub struct WindowsCrashCapture {
    owner: CoordinatorKey,
    shared: Option<Arc<Shared>>,
    port: Arc<dyn NativePort>,
    start: Option<WorkJob>,
    close: Option<WorkJob>,
    watch: Option<WatchJob>,
    snapshot: CaptureSnapshot,
    errors: [Option<CrashError>; 9],
    omitted: u64,
    pending_registration: bool,
    close_deadline: Option<Moment>,
    start_budget: Option<CaptureBudget>,
    execution: Option<sugarsubstitute_execution::ExecutionClient>,
    execution_id: Option<sugarsubstitute_contracts::execution::ExecutionId>,
}
impl WindowsCrashCapture {
    /// Construct an inert owner without loading a DLL or starting a process.
    /// Native registration is attempted at most once for this owner/process.
    pub fn new(owner: CoordinatorKey) -> Self {
        Self::with_port(owner, crate::native::port())
    }
    fn with_port(owner: CoordinatorKey, port: Arc<dyn NativePort>) -> Self {
        Self {
            owner,
            shared: None,
            port,
            start: None,
            close: None,
            watch: None,
            snapshot: CaptureSnapshot {
                owner,
                state: CaptureState::Dormant,
                handler: None,
                handler_exit: None,
                callback_pin_bytes: 0,
                callback_pin_reservation_bytes: 0,
            },
            errors: std::array::from_fn(|_| None),
            omitted: 0,
            pending_registration: false,
            close_deadline: None,
            start_budget: None,
            execution: None,
            execution_id: None,
        }
    }
    /// Read immutable cached facts without blocking or entering native code.
    pub const fn snapshot(&self) -> CaptureSnapshot {
        self.snapshot
    }
    /// Borrow original bounded causes in occurrence order; formatting is redacted.
    pub fn errors(&self) -> impl Iterator<Item = &CrashError> {
        self.errors.iter().flatten()
    }
    /// Number of additional causes omitted after the fixed nine retained slots.
    pub const fn omitted_error_count(&self) -> u64 {
        self.omitted
    }
    fn retain(&mut self, cause: CrashError) {
        if self.errors.iter().flatten().any(|old| old.same_cause(&cause)) {
            return;
        }
        if let Some(slot) = self.errors.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}
