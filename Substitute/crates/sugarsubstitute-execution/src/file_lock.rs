// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Wait for whole-file byte-range locks on a blocking job, bounded and cancellable.
//!
//! The wait blocks only the calling blocking-pool worker by driving one small future on the
//! execution runtime that admitted the job. The lock request itself is a real overlapped
//! `LockFileEx`: it completes in the kernel as soon as the conflicting holder releases, and a
//! deadline or cancellation cancels it and observes its completion before returning.

use std::{error::Error, fmt, io, os::windows::io::BorrowedHandle};
use sugarsubstitute_contracts::execution::{JobClass, JobContext, Moment};

/// Whole-file byte-range lock mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockKind {
    /// Coexists with other shared holders and excludes every exclusive holder.
    Shared,
    /// Excludes every other shared and exclusive holder.
    Exclusive,
}

/// Why a lock wait ended without granting the lock. No lock is held after any of these.
#[derive(Debug)]
pub enum FileLockError {
    /// Another handle kept a conflicting lock until the wait's deadline on the job's clock.
    Timeout,
    /// The job's cancellation was requested before the lock was granted.
    Cancelled,
    /// The calling job is not a blocking infrastructure job, so it may not block its worker.
    Unavailable,
    /// The native lock request or its completion failed; the original cause is preserved.
    Io(io::Error),
}
impl fmt::Display for FileLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Timeout => "File lock stayed held by another owner until the wait deadline",
            Self::Cancelled => "File lock wait was cancelled",
            Self::Unavailable => "File lock waits require a blocking infrastructure job",
            Self::Io(_) => "File lock request failed",
        })
    }
}
impl Error for FileLockError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(cause) => Some(cause),
            Self::Timeout | Self::Cancelled | Self::Unavailable => None,
        }
    }
}

/// Acquire a whole-range lock of `kind` on `file`, waiting no later than `until`.
///
/// The effective bound is the earlier of `until` and the job deadline, both on the job's
/// injected clock. Call only from a blocking infrastructure job (`submit_blocking`); the call
/// blocks that worker and must never run on a UI thread or inside an async task. `file` must be
/// a handle opened for overlapped I/O; the granted lock belongs to it until it unlocks or
/// closes. The handle stays borrowed for the whole call, including cancellation completion.
/// # Errors
/// `Timeout` at the bound, `Cancelled` on a cancellation request, `Unavailable` outside a
/// blocking infrastructure job, or `Io` with the native cause (a synchronous handle is rejected
/// as `InvalidInput` before any request). No lock is held after an error.
pub fn lock(
    context: &JobContext,
    file: BorrowedHandle<'_>,
    kind: LockKind,
    until: Moment,
) -> Result<(), FileLockError> {
    if !matches!(
        context.spec().class,
        JobClass::InfrastructureOrdinary | JobClass::InfrastructureLifecycle
    ) {
        return Err(FileLockError::Unavailable);
    }
    if context.cancellation_requested() {
        return Err(FileLockError::Cancelled);
    }
    let until = context.deadline().map_or(until, |deadline| deadline.min(until));
    native::lock(context, file, kind, until)
}

/// What ended one wait race on the execution runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ended {
    Granted,
    Deadline,
    Cancelled,
}

/// Drive `granted` against the deadline and cancellation on the admitting runtime.
/// Cancellation and an elapsed deadline are observed before the first poll of `granted`.
fn race(
    granted: impl std::future::Future<Output = ()>,
    until: Moment,
    context: &JobContext,
) -> Result<Ended, FileLockError> {
    if context.cancellation_requested() {
        return Ok(Ended::Cancelled);
    }
    if context.now() >= until {
        return Ok(Ended::Deadline);
    }
    let runtime = tokio::runtime::Handle::try_current()
        .map_err(|cause| FileLockError::Io(io::Error::other(cause)))?;
    let deadline = context.sleep_until(until);
    let cancelled = context.cancelled();
    Ok(runtime.block_on(async move {
        tokio::select! {
            biased;
            () = cancelled => Ended::Cancelled,
            () = granted => Ended::Granted,
            () = deadline => Ended::Deadline,
        }
    }))
}

#[allow(unsafe_code)]
#[path = "file_lock_native.rs"]
mod native;

#[cfg(test)]
#[path = "file_lock_tests.rs"]
mod tests;
