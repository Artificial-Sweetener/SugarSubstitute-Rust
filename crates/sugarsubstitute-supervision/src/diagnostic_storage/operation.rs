// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind exact operations and original normal/cleanup deadlines across all storage calls.
use super::*;
pub(super) fn check(
    state: &mut State,
    key: LogicalStoreOperationKey,
    context: &JobContext,
    cleanup: bool,
) -> Result<(), DiagnosticStorageError> {
    let binding = state.operation.as_mut().ok_or_else(|| error(key, Code::Closed))?;
    if binding.key != key || binding.execution != context.owner_id() {
        return Err(error(key, Code::InvalidIdentity));
    }
    let deadline = context.deadline().ok_or_else(|| error(key, Code::Admission))?;
    let now = context.now();
    if now < binding.last_observed {
        state.closed = true;
        return Err(error(key, Code::Deadline));
    }
    binding.last_observed = now;
    state.last_observed = Some(now);
    if now >= deadline {
        return Err(error(key, Code::Deadline));
    }
    if cleanup
        && binding.cleanup.is_none()
        && deadline.as_nanos() - now.as_nanos() > DIAGNOSTIC_CLEANUP_NANOS
    {
        return Err(error(key, Code::Admission));
    }
    if context.cancellation_requested() {
        return Err(error(key, Code::Cancelled));
    }
    let limit = if cleanup {
        let scope = binding.cleanup_scope.get_or_insert(context.scope_id());
        if *scope != context.scope_id() {
            return Err(error(key, Code::InvalidIdentity));
        }
        let limit = binding.cleanup.get_or_insert(deadline);
        if deadline > *limit {
            return Err(error(key, Code::Deadline));
        }
        *limit = (*limit).min(deadline);
        *limit
    } else {
        if state.closed {
            return Err(error(key, Code::Closed));
        }
        if binding.scope != context.scope_id() {
            return Err(error(key, Code::InvalidIdentity));
        }
        if deadline > binding.deadline {
            return Err(error(key, Code::Deadline));
        }
        binding.deadline = binding.deadline.min(deadline);
        binding.deadline
    };
    if now >= limit {
        return Err(error(key, Code::Deadline));
    }
    Ok(())
}
pub(super) fn next_lease(
    state: &mut State,
    key: LogicalStoreOperationKey,
) -> Result<u64, DiagnosticStorageError> {
    let id = state.next_lease;
    state.next_lease = id.checked_add(1).ok_or_else(|| error(key, Code::Capacity))?;
    Ok(id)
}
