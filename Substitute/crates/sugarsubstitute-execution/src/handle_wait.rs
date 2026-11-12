// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Wait until the first of several kernel objects is signalled, bounded and cancellable.
//!
//! A process creator uses this to observe a child's "took over" event or its exit without
//! polling. The wait blocks only the calling blocking-pool worker by driving one small future on
//! the execution runtime that admitted the job; every native registration is retired before the
//! call returns.

use std::{error::Error, fmt, io, os::windows::io::BorrowedHandle};
use sugarsubstitute_contracts::execution::{JobClass, JobContext, Moment};

/// Most kernel objects one wait may observe; creators watch a child and one event.
pub const MAX_HANDLES: usize = 4;

/// Why a wait ended without any object being signalled.
#[derive(Debug)]
pub enum HandleWaitError {
    /// No object was signalled before the deadline on the job's clock.
    Timeout,
    /// The job's cancellation was requested first.
    Cancelled,
    /// The calling job is not a blocking infrastructure job, so it may not block its worker.
    Unavailable,
    /// No objects, too many, or a native registration failure; the cause is preserved.
    Io(io::Error),
}
impl fmt::Display for HandleWaitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Timeout => "No awaited object was signalled before the wait deadline",
            Self::Cancelled => "Object wait was cancelled",
            Self::Unavailable => "Object waits require a blocking infrastructure job",
            Self::Io(_) => "Object wait failed",
        })
    }
}
impl Error for HandleWaitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(cause) => Some(cause),
            Self::Timeout | Self::Cancelled | Self::Unavailable => None,
        }
    }
}

/// Return the index of the first signalled object in `handles`, waiting no later than `until`.
///
/// The effective bound is the earlier of `until` and the job deadline, both on the job's injected
/// clock. An object already signalled is reported without waiting; when several are signalled the
/// lowest index wins, so callers list the outcome they prefer first. Call only from a blocking
/// infrastructure job; the call blocks that worker and must never run on a UI thread. Every
/// handle stays borrowed for the whole call and must refer to an object that stays signalled once
/// signalled, such as a manual-reset event or a process; an auto-reset event would be consumed by
/// the wait and is refused as an `Io` error after waking.
/// # Errors
/// `Timeout`, `Cancelled`, `Unavailable` outside a blocking infrastructure job, or `Io` for an
/// empty or oversized list or a native failure. No registration outlives an error.
pub fn wait_any(
    context: &JobContext,
    handles: &[BorrowedHandle<'_>],
    until: Moment,
) -> Result<usize, HandleWaitError> {
    if !matches!(
        context.spec().class,
        JobClass::InfrastructureOrdinary | JobClass::InfrastructureLifecycle
    ) {
        return Err(HandleWaitError::Unavailable);
    }
    if handles.is_empty() || handles.len() > MAX_HANDLES {
        return Err(HandleWaitError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Object waits observe between one and four handles",
        )));
    }
    if let Some(index) = native::signalled(handles)? {
        return Ok(index);
    }
    if context.cancellation_requested() {
        return Err(HandleWaitError::Cancelled);
    }
    let until = context.deadline().map_or(until, |deadline| deadline.min(until));
    native::wait(context, handles, until)
}

/// What ended one wait race on the execution runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ended {
    Signalled,
    Deadline,
    Cancelled,
}

/// Drive `signalled` against the deadline and cancellation on the admitting runtime.
fn race(
    signalled: impl std::future::Future<Output = ()>,
    until: Moment,
    context: &JobContext,
) -> Result<Ended, HandleWaitError> {
    if context.cancellation_requested() {
        return Ok(Ended::Cancelled);
    }
    if context.now() >= until {
        return Ok(Ended::Deadline);
    }
    let runtime = tokio::runtime::Handle::try_current()
        .map_err(|cause| HandleWaitError::Io(io::Error::other(cause)))?;
    let deadline = context.sleep_until(until);
    let cancelled = context.cancelled();
    Ok(runtime.block_on(async move {
        tokio::select! {
            biased;
            () = cancelled => Ended::Cancelled,
            () = signalled => Ended::Signalled,
            () = deadline => Ended::Deadline,
        }
    }))
}

#[allow(unsafe_code)]
#[path = "handle_wait_native.rs"]
mod native;

#[cfg(test)]
#[allow(unsafe_code)]
#[path = "handle_wait_tests.rs"]
mod tests;
