// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind exact operations and original normal/cleanup deadlines across all storage calls.
use super::*;
pub(super) fn check(
    state: &mut StateAccess<'_>,
    key: LogicalStoreOperationKey,
    context: &JobContext,
    cleanup: bool,
) -> Result<(), DiagnosticStorageError> {
    state.check(key, context, cleanup)
}
pub(super) struct ControlObservation {
    original_now: Option<Moment>,
    now: Moment,
    original_cancelled: bool,
    cancelled: bool,
}
impl ControlObservation {
    pub(super) fn sample(original: Option<&JobContext>, context: &JobContext) -> Self {
        Self {
            original_now: original.map(JobContext::now),
            now: context.now(),
            original_cancelled: original.is_some_and(JobContext::cancellation_requested),
            cancelled: context.cancellation_requested(),
        }
    }
}
pub(super) fn check_binding_observed(
    binding: &mut Binding,
    last_observed: &mut Option<Moment>,
    closed: &mut bool,
    key: LogicalStoreOperationKey,
    context: &JobContext,
    cleanup: bool,
    observation: ControlObservation,
) -> Result<(), DiagnosticStorageError> {
    if binding.key != key || binding.execution != context.owner_id() {
        return Err(error(key, Code::InvalidIdentity));
    }
    let deadline = context.deadline().ok_or_else(|| error(key, Code::Admission))?;
    if (!cleanup && binding.scope != context.scope_id())
        || (cleanup && binding.cleanup_scope.is_some_and(|scope| scope != context.scope_id()))
    {
        return Err(error(key, Code::InvalidIdentity));
    }
    let ControlObservation { original_now, now, original_cancelled, cancelled } = observation;
    if original_now.is_some_and(|original| original < binding.last_observed || now < original) {
        *closed = true;
        return Err(error(key, Code::Deadline));
    }
    if now < binding.last_observed {
        *closed = true;
        return Err(error(key, Code::Deadline));
    }
    binding.last_observed = now;
    *last_observed = Some(now);
    if now >= deadline {
        return Err(error(key, Code::Deadline));
    }
    if cleanup
        && binding.cleanup.is_none()
        && deadline.as_nanos() - now.as_nanos() > DIAGNOSTIC_CLEANUP_NANOS
    {
        return Err(error(key, Code::Admission));
    }
    if cancelled {
        return Err(error(key, Code::Cancelled));
    }
    if original_cancelled {
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
        if binding.cleanup_context.is_none() {
            binding.cleanup_context = Some(context.clone());
        }
        *limit = (*limit).min(deadline);
        *limit
    } else {
        if *closed {
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
    state: &mut StateAccess<'_>,
    key: LogicalStoreOperationKey,
) -> Result<u64, DiagnosticStorageError> {
    let id = state.data_mut()?.next_lease;
    state.data_mut()?.next_lease = id.checked_add(1).ok_or_else(|| error(key, Code::Capacity))?;
    Ok(id)
}
