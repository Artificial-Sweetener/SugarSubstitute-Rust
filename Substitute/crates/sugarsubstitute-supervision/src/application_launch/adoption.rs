// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep a trial child's run handoff open until the child holds its own run slot or is gone.
//!
//! Windows releases a process's byte-range locks when it exits, so a trial creator cannot hand
//! run exclusion to its child and leave at once: it keeps its own slot and the handoff byte until
//! the child signals the slot-held event, or the child's process ends. A child silent past its
//! bound loses the handoff, gets a short grace to report an adoption that just happened, and is
//! then ended, so the creator never leaves a child that might still take the trial behind it.
//! Every wait is an execution-owned kernel wait on the job's clock, never a poll.
use crate::{ClaimError, setup_storage::UnadoptedEnding};
use std::{
    os::windows::io::{AsRawHandle, BorrowedHandle},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
use sugarsubstitute_execution::handle_wait::{HandleWaitError, wait_any};
use windows::Win32::{Foundation::HANDLE, System::Threading::TerminateProcess};

/// Bounds of one adoption wait on the job's clock.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    /// From creation until the child must have signalled or exited.
    pub(super) adoption: Duration,
    /// After the handoff is given up, for a last-moment adoption or a refusal to show.
    pub(super) grace: Duration,
    /// From termination until the child's exit must be observed.
    pub(super) termination: Duration,
}
impl Bounds {
    /// Product bounds: the child adopts at entry before verifying payloads.
    pub(super) const TRIAL: Self = Self {
        adoption: crate::setup_storage::TRIAL_ADOPTION,
        grace: crate::setup_storage::TRIAL_ADOPTION_GRACE,
        termination: crate::setup_storage::TRIAL_ADOPTION,
    };
}

/// How a created trial child ended its creator's wait.
#[derive(Debug)]
pub(super) enum Adoption {
    /// The child holds its own run slot; the creator may release everything and leave.
    Adopted,
    /// The child is gone without having held its slot; the creator still holds the run.
    Unadopted(UnadoptedEnding),
    /// Whether the child holds the trial is unknown: the wait failed or was cancelled, or a
    /// silent child could not be ended. The handoff has been given up.
    Uncertain(ClaimError),
}

/// Wait for `adopted` or `process`, giving up the handoff through `release_handoff` before any
/// outcome other than adoption that leaves the child possibly alive. Run on the blocking
/// infrastructure job that created the child, while the creator still holds the handoff.
pub(super) fn await_adoption(
    context: &JobContext,
    adopted: BorrowedHandle<'_>,
    process: BorrowedHandle<'_>,
    release_handoff: impl FnOnce(),
    bounds: Bounds,
) -> Adoption {
    let first = after(context, bounds.adoption)
        .and_then(|until| observe(context, adopted, process, until, UnadoptedEnding::Exited));
    match first {
        Ok(Some(adoption)) => return adoption,
        Ok(None) => {}
        Err(cause) => {
            release_handoff();
            return Adoption::Uncertain(cause);
        }
    }
    release_handoff();
    tracing::warn!(
        operation = "application_launch_trial_silent",
        "The updated release did not take over in time; giving up its handoff"
    );
    let grace = after(context, bounds.grace)
        .and_then(|until| observe(context, adopted, process, until, UnadoptedEnding::Ended));
    match grace {
        Ok(Some(adoption)) => adoption,
        Ok(None) => end(context, process, bounds.termination),
        Err(cause) => Adoption::Uncertain(cause),
    }
}

/// One bounded wait: adoption wins over exit when both are signalled; `None` at the bound.
fn observe(
    context: &JobContext,
    adopted: BorrowedHandle<'_>,
    process: BorrowedHandle<'_>,
    until: Moment,
    exited: UnadoptedEnding,
) -> Result<Option<Adoption>, ClaimError> {
    match wait_any(context, &[adopted, process], until) {
        Ok(0) => Ok(Some(Adoption::Adopted)),
        Ok(_) => Ok(Some(Adoption::Unadopted(exited))),
        Err(HandleWaitError::Timeout) => Ok(None),
        Err(cause) => Err(ClaimError::new("application_launch_adoption_wait", cause)),
    }
}

/// End a child that never adopted and no longer can, and observe its exit before returning.
fn end(context: &JobContext, process: BorrowedHandle<'_>, bound: Duration) -> Adoption {
    // SAFETY: the creator owns this live process handle; the child never took over the trial
    // and its handoff is gone, so it can no longer adopt.
    if let Err(cause) = unsafe { TerminateProcess(HANDLE(process.as_raw_handle()), 1) } {
        return Adoption::Uncertain(ClaimError::new("application_launch_trial_terminate", cause));
    }
    match after(context, bound).and_then(|until| {
        wait_any(context, &[process], until)
            .map_err(|cause| ClaimError::new("application_launch_trial_terminated", cause))
    }) {
        Ok(_) => Adoption::Unadopted(UnadoptedEnding::Ended),
        Err(cause) => Adoption::Uncertain(cause),
    }
}

fn after(context: &JobContext, bound: Duration) -> Result<Moment, ClaimError> {
    context
        .now()
        .checked_add(bound)
        .map_err(|cause| ClaimError::new("application_launch_adoption_bound", cause))
}

#[cfg(test)]
mod tests;
