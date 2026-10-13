// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe current actual record cancellation and bounded complete scope ancestry.
use crate::admission::{Core, Record, State};
use sugarsubstitute_contracts::execution::{AdmissionError, CancelControl, Moment};
/// Pure actual-state validation; no provider, context reconstruction or native authority.
pub(crate) fn scope_deadline(
    core: &Core,
    state: &State,
    record: &Record,
) -> Result<Option<Moment>, AdmissionError> {
    let ancestry = scope_ancestry_deadline(core, state, record)?;
    Ok(crate::scope::earliest(record.deadline, ancestry))
}

/// Observe only actual scope/cancellation ownership after a finite job has completed.
/// Callers must separately prove their retained native lifetime and original record.
/// This creates no fresh effect authority and changes no original job deadline.
pub(crate) fn scope_ancestry_deadline(
    core: &Core,
    state: &State,
    record: &Record,
) -> Result<Option<Moment>, AdmissionError> {
    if state.stopping || core.clock.failed() {
        return Err(AdmissionError::ShuttingDown);
    }
    if record.job.cancellation().is_requested() {
        return Err(AdmissionError::ScopeClosed { scope_id: record.scope_id });
    }
    let mut cutoff = None;
    let mut current = Some(record.scope_id);
    let mut ancestors = 0;
    while let Some(id) = current {
        // Both currently supported scope partitions may occur in one genuine ancestry.
        if ancestors >= 64 + 132 {
            return Err(AdmissionError::ScopeClosed { scope_id: record.scope_id });
        }
        ancestors += 1;
        let scope = state
            .scopes
            .get(&id)
            .ok_or(AdmissionError::ScopeClosed { scope_id: record.scope_id })?;
        if scope.closed || scope.abandoned || scope.cancel.is_requested() {
            return Err(AdmissionError::ScopeClosed { scope_id: record.scope_id });
        }
        cutoff = crate::scope::earliest(cutoff, scope.deadline);
        current = scope.parent;
    }
    Ok(cutoff)
}
