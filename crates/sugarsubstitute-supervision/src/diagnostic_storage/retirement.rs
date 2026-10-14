// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original cleanup bounds and expose actual outstanding custody.
use super::*;

/// Borrow the actual accepted cleanup binding while its original owner retires.
/// This grants no allocation, normal admission, or independent settlement proof.
pub(crate) struct DiagnosticNativeRetirement<'a> {
    shared: Shared,
    key: LogicalStoreOperationKey,
    current: &'a JobContext,
}
impl DiagnosticNativeRetirement<'_> {
    pub(super) fn new(
        shared: Shared,
        key: LogicalStoreOperationKey,
        current: &JobContext,
    ) -> DiagnosticNativeRetirement<'_> {
        DiagnosticNativeRetirement { shared, key, current }
    }

    /// Sample original/live providers outside State, then revalidate the latched binding.
    pub(crate) fn check(
        &mut self,
        original: &allocation::DiagnosticNativeGrant,
    ) -> Result<(), allocation::DiagnosticNativeAllocationFailure> {
        self.observe(original, true).map_err(|failure| {
            allocation::DiagnosticNativeAllocationFailure::rejected(Some(self.key), failure.code())
        })
    }
    fn observe(
        &mut self,
        original: &allocation::DiagnosticNativeGrant,
        admitted: bool,
    ) -> Result<(), DiagnosticStorageError> {
        let state = lock(&self.shared);
        if !original.matches_allocation(&state.allocation) {
            return Err(error(self.key, Code::InvalidIdentity));
        }
        let binding = state.operation.as_ref().ok_or_else(|| error(self.key, Code::Closed))?;
        if binding.key != self.key
            || binding.execution != self.current.owner_id()
            || (admitted && (!state.closed || binding.cleanup.is_none()))
        {
            return Err(error(self.key, Code::InvalidIdentity));
        }
        let first = binding.cleanup_context.clone();
        drop(state);
        let observation = operation::ControlObservation::sample(first.as_ref(), self.current);
        let mut state = lock(&self.shared);
        if !original.matches_allocation(&state.allocation) {
            return Err(error(self.key, Code::InvalidIdentity));
        }
        let State { operation: binding, last_observed, closed, .. } = &mut *state;
        let binding = binding.as_mut().ok_or_else(|| error(self.key, Code::Closed))?;
        let result = operation::check_binding_observed(
            binding,
            last_observed,
            closed,
            self.key,
            self.current,
            true,
            observation,
        );
        drop(state);
        drop(first);
        result
    }
}

/// Restore the same native capsule after normal return or unwinding.
struct RetiringPort {
    owned: Option<OwnedPort>,
    shared: Shared,
    key: LogicalStoreOperationKey,
}
impl RetiringPort {
    fn restore(&mut self) -> Result<(), DiagnosticStorageError> {
        let mut state = lock(&self.shared);
        if state.port.is_some() {
            return Err(error(self.key, Code::Busy));
        }
        state.port = self.owned.take();
        state.retiring_handles = 0;
        state.retiring_files = 0;
        Ok(())
    }
}
impl Drop for RetiringPort {
    fn drop(&mut self) {
        if self.owned.is_some() && self.restore().is_err() {
            // A conflicting owner cannot authorize destruction of the detached capsule.
            if let Some(owned) = self.owned.take() {
                std::mem::forget(owned);
            }
        }
    }
}

/// Observe owner admission and actual custody without conferring mutation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticStorageSnapshot {
    execution: Option<ExecutionId>,
    deadline: Option<Moment>,
    cleanup: Option<Moment>,
    closed: bool,
    writer: bool,
    handles: usize,
    stages: usize,
    reads: usize,
    disposition: bool,
    retirement_in_progress: bool,
}
impl DiagnosticStorageSnapshot {
    /// Observe the execution owner latched by the first operation.
    pub const fn execution(self) -> Option<ExecutionId> {
        self.execution
    }
    /// Observe the current original or shortened finite operation deadline.
    pub const fn deadline(self) -> Option<Moment> {
        self.deadline
    }
    /// Observe the first independently admitted cleanup deadline, never renewed.
    pub const fn cleanup_deadline(self) -> Option<Moment> {
        self.cleanup
    }
    /// Observe whether ordinary admission has closed.
    pub const fn admission_closed(self) -> bool {
        self.closed
    }
    /// Observe actual held writer exclusion.
    pub const fn writer_held(self) -> bool {
        self.writer
    }
    /// Observe the last actual live handle count, retained conservatively during detached native work.
    pub const fn live_handles(self) -> usize {
        self.handles
    }
    /// Observe whether native retirement is running outside the storage mutex.
    pub const fn retirement_in_progress(self) -> bool {
        self.retirement_in_progress
    }
    /// Observe owner-retained stage slots, including uncertain creations.
    pub const fn stages(self) -> usize {
        self.stages
    }
    /// Observe owner-retained read slots, including uncertain opens.
    pub const fn reads(self) -> usize {
        self.reads
    }
    /// Observe whether an unsettled disposition still owns custody.
    pub const fn disposition_retained(self) -> bool {
        self.disposition
    }
}
impl DiagnosticStorage {
    /// Close admission and checked-retire all actual outputs under original cleanup.
    ///
    /// Persistent bytes remain intact. A successful close abandons uncommitted stages
    /// to bounded next-launch recovery; it does not describe them as reclaimed bytes.
    /// # Errors
    /// Failed context, unlock or close preserves actual native custody and slots.
    pub fn retire(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        if !Shared::ptr_eq(&self.shared, &operation.shared) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let mut access = StateAccess::acquire(&self.shared, operation.key);
        access.check(operation.key, context, true)?;
        if access.data_mut()?.capture_reads != 0
            || access.data_mut()?.finished_owner.is_some()
            || access.data_mut()?.capture.is_some()
        {
            return Err(error(operation.key, Code::Busy));
        }
        let original = access.data_mut()?.allocation.native_grant();
        let mut control = DiagnosticNativeRetirement {
            shared: self.shared.clone(),
            key: operation.key,
            current: context,
        };
        let state = access.data_mut()?;
        let owned = state.port.take().ok_or_else(|| error(operation.key, Code::Busy))?;
        state.retiring_handles = owned.live_handles();
        state.retiring_files = owned.retained_files();
        state.closed = true;
        state.allocation.close();
        drop(access);
        let mut port =
            RetiringPort { owned: Some(owned), shared: self.shared.clone(), key: operation.key };
        let result = port
            .owned
            .as_mut()
            .ok_or_else(|| error(operation.key, Code::Closed))?
            .borrow(Some(operation.key))?
            .retire(context, &mut control)
            .map_err(|cause| native(operation.key, Code::Retirement, cause));
        port.restore()?;
        result?;
        if lock(&self.shared).live_handles() != 0 {
            return Err(error(operation.key, Code::Retirement));
        }
        control.observe(&original, true)?;
        let mut state = lock(&self.shared);
        state.stages.clear();
        state.reads.clear();
        state.deletion = None;
        state.migration = None;
        state.payload = None;
        state.writer = false;
        state.inventory_complete = false;
        Ok(())
    }
    /// Observe exact live custody and latched budgets without executing native work.
    pub fn snapshot(&self) -> DiagnosticStorageSnapshot {
        let state = lock(&self.shared);
        DiagnosticStorageSnapshot {
            execution: state.execution,
            deadline: state.operation.as_ref().map(|value| value.deadline),
            cleanup: state.operation.as_ref().and_then(|value| value.cleanup),
            closed: state.closed,
            writer: state.writer,
            handles: state.live_handles(),
            retirement_in_progress: state.port.is_none() && state.closed,
            stages: state.stage_slots(),
            reads: state.reads.len() + state.capture_reads,
            disposition: state.deletion.is_some()
                || state.migration.is_some()
                || state.payload.is_some()
                || state.capture.is_some(),
        }
    }
}
