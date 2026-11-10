// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain first projection failure, original controls and complete charged source custody.
use super::*;
use std::fmt;
/// Preserve the original failure and frame through late cancellation or rejection.
pub(in crate::production::entry_activation) struct ProjectionFailure {
    pub(super) retained: ChargedBlockingInput<ProjectionInput>,
}
/// Return a completed, unlaunched request's passive connection loan during
/// shutdown. The complete outcome, first error, controls, metadata and original
/// charges remain with the caller. This proves no transport/native settlement.
/// An admitted captured session owns its transferred source and is excluded.
pub(in crate::production::entry_activation) fn return_unused_source(
    outcome: &mut JobOutcome<ProjectedEntry, ProjectionFailure>,
) {
    match outcome {
        JobOutcome::Success(value) => drop(value.snapshot.take()),
        JobOutcome::Failure(failure) => {
            let input = failure.retained.input_mut();
            drop(input.request.take());
            drop(input.snapshot.take());
        }
        _ => {}
    }
}

pub(super) enum ProjectionCause {
    NotStarted,
    Claim(MemoryClaimError),
    Funding(sugarsubstitute_contracts::lifecycle::LifecycleRejection),
    Operation(Box<dyn Error + Send + Sync>),
}
impl ProjectionCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotStarted => None,
            Self::Claim(cause) => Some(cause),
            Self::Funding(cause) => Some(cause),
            Self::Operation(cause) => Some(cause.as_ref()),
        }
    }
}
impl fmt::Debug for ProjectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryProjectionFailure")
            .field("post_control", &self.retained.input().post)
            .field("original_retained", &self.retained.input().request.is_some())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ProjectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Authenticated entry preparation failed")
    }
}
impl Error for ProjectionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.retained.input().cause.source()
    }
}
#[derive(Debug)]
pub(in crate::production::entry_activation) enum ControlFailure {
    Cancelled,
    Expired,
    ClockRegression,
}
impl fmt::Display for ControlFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entry projection control: {self:?}")
    }
}
impl Error for ControlFailure {}
pub(in crate::production::entry_activation) fn check(
    context: &JobContext,
    received: Moment,
    until: Moment,
) -> Result<(), ControlFailure> {
    let now = context.now();
    if context.cancellation_requested() {
        return Err(ControlFailure::Cancelled);
    }
    if now < received {
        return Err(ControlFailure::ClockRegression);
    }
    if now >= until || context.deadline().is_none_or(|deadline| now >= deadline || deadline > until)
    {
        return Err(ControlFailure::Expired);
    }
    Ok(())
}
