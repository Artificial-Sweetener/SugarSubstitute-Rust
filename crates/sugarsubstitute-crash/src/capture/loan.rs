// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return the identical retained lease after finite work or provider unwind.
use super::*;
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::Moment};

/// Stack custody on the actual admitted worker; restoration performs no FFI.
pub(super) struct NativeLoan {
    shared: Arc<Shared>,
    lease: Option<Box<dyn CaptureLease>>,
}
impl NativeLoan {
    /// Install an actual newly created owner before its first external effect.
    pub(super) fn install(
        shared: Arc<Shared>,
        lease: Box<dyn CaptureLease>,
        deadline: Option<Moment>,
    ) -> Result<Self, CrashError> {
        let mut state = lock(&shared.native);
        if state.borrowed || state.lease.is_some() {
            return Err(CrashError::invalid(CaptureOperation::Registration));
        }
        state.lease = Some(lease);
        state.deadline = deadline;
        let lease = state.lease.take();
        state.borrowed = true;
        drop(state);
        Ok(Self { shared, lease })
    }
    /// Claim one exclusive existing lease; an in-flight loan is never vacancy.
    pub(super) fn take(
        shared: Arc<Shared>,
        operation: CaptureOperation,
    ) -> Result<Option<Self>, CrashError> {
        let mut state = lock(&shared.native);
        if state.borrowed {
            return Err(CrashError::invalid(operation));
        }
        let Some(lease) = state.lease.take() else {
            return Ok(None);
        };
        state.borrowed = true;
        drop(state);
        Ok(Some(Self { shared, lease: Some(lease) }))
    }
    /// Borrow the same concrete owner without holding a state/lifetime mutex.
    pub(super) fn lease(&mut self) -> Result<&mut (dyn CaptureLease + 'static), CrashError> {
        self.lease.as_deref_mut().ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))
    }
    /// Drain native causes outside locks, retaining each in the original ledger.
    pub(super) fn drain_errors(&mut self) -> Result<(), CrashError> {
        loop {
            let Some(cause) = self.lease()?.take_error() else {
                break;
            };
            lock(&self.shared.native).retain(cause);
        }
        let omitted = self.lease()?.take_omitted_errors();
        let mut state = lock(&self.shared.native);
        state.omitted = state.omitted.saturating_add(omitted);
        Ok(())
    }
}
impl Drop for NativeLoan {
    fn drop(&mut self) {
        // Only install/take write the lease slot and both reject borrowed state.
        // The Arc keeps the original cell alive through callback/panic unwind.
        let mut state = lock(&self.shared.native);
        state.lease = self.lease.take();
        state.borrowed = false;
    }
}
