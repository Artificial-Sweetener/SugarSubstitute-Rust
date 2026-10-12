// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe external time before fencing exact current capture ownership and native acceptance.
use super::{Shared, lock};
use crate::native::NativeObservationError;
use sugarsubstitute_contracts::execution::{JobContext, JobPhase};

pub(crate) enum Dispatch<T> {
    Issued(T),
    Stopped,
    Expired,
}

/// Run only nonblocking native acceptance/adoption under the actual custody fence.
/// The supplied clock may synchronously request cancellation before this fence.
pub(crate) fn execute<T>(
    shared: &Shared,
    context: &JobContext,
    native: impl FnOnce() -> Result<T, NativeObservationError>,
) -> Result<Dispatch<T>, NativeObservationError> {
    let core = shared.custody.core.upgrade().ok_or_else(|| invariant("capture_issue_owner"))?;
    let observed = match core.clock.observe_unlatched() {
        Ok(observed) => observed,
        Err(payload) => core.clock.fail_observation(payload),
    };
    let state = lock(&core.state);
    let record = state.jobs.get(&shared.custody.job);
    let current = context.owner_id() == core.id
        && shared.custody.execution == core.id
        && record.is_some_and(|record| {
            !record.retiring
                && !matches!(record.phase, JobPhase::Terminal(_))
                && record.scope_id == context.scope_id()
                && record.spec.id == context.job_id()
                && record
                    .native_custody
                    .as_ref()
                    .is_some_and(|custody| std::sync::Arc::ptr_eq(custody, &shared.custody))
        });
    let deadlines = record.and_then(|record| record.native_deadlines);
    let policy_deadline = record.and_then(|record| record.deadline);
    let stopping = state.stopping;
    let custody = lock(&shared.custody.state);
    drop(state);
    if !current {
        return Err(invariant("capture_issue_context"));
    }
    if stopping
        || custody.stopped
        || context.cancellation_requested()
        || core.clock.failed()
        || deadlines.is_none_or(|deadlines| deadlines.stopped())
    {
        return Ok(Dispatch::Stopped);
    }
    if observed >= shared.custody.attach_until
        || policy_deadline.is_some_and(|until| observed >= until)
        || deadlines.is_some_and(|deadlines| deadlines.expired(observed))
    {
        return Ok(Dispatch::Expired);
    }
    let accepted = native()?;
    drop(custody);
    Ok(Dispatch::Issued(accepted))
}
fn invariant(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(
        stage,
        std::io::Error::other("Capture native custody invariant rejected"),
    )
}
