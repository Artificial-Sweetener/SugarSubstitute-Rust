// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original cleanup bounds and expose actual outstanding custody.
use super::*;

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
    /// Observe actual live native handles, including partial outputs.
    pub const fn live_handles(self) -> usize {
        self.handles
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
        if !Arc::ptr_eq(&self.shared, &operation.shared) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let mut state = lock(&self.shared);
        check(&mut state, operation.key, context, true)?;
        state.closed = true;
        state
            .port
            .retire(context)
            .map_err(|cause| native(operation.key, Code::Retirement, cause))?;
        if state.port.live_handles() != 0 {
            return Err(error(operation.key, Code::Retirement));
        }
        check(&mut state, operation.key, context, true)?;
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
            handles: state.port.live_handles(),
            stages: state.stages.len(),
            reads: state.reads.len(),
            disposition: state.deletion.is_some()
                || state.migration.is_some()
                || state.payload.is_some(),
        }
    }
}
