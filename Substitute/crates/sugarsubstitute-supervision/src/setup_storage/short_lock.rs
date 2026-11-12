// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound reader-writer waits on the short selection and trust lock files.
//!
//! Readers take a shared whole-file lock for one consistent read; writers take it exclusive for
//! one compare-and-publish. Both wait in the kernel through the execution owner, never with a
//! caller-side retry, and give up after [`WAIT`] or at the job deadline, whichever is first.
use std::{fs::File, os::windows::io::AsHandle, time::Duration};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
pub(super) use sugarsubstitute_execution::file_lock::LockKind;
use sugarsubstitute_execution::file_lock::{self, FileLockError};

/// Longest wait for another short holder. Holders keep these locks only for a bounded read or
/// one record publication, so a longer wait means a holder is stuck, not busy.
pub(super) const WAIT: Duration = Duration::from_secs(5);

/// Outcome of a short lock wait that did not grant the lock.
pub(super) enum Refused {
    /// The other holder kept the lock past [`WAIT`] or the job deadline.
    Timeout,
    /// The job was cancelled while waiting.
    Cancelled,
    /// The native request failed, or the job may not block.
    Storage(std::io::Error),
}

/// Lock `file`, which must be opened for overlapped I/O, and keep it locked until it closes.
pub(super) fn acquire(file: &File, kind: LockKind, context: &JobContext) -> Result<(), Refused> {
    let until = context.now().checked_add(WAIT).unwrap_or(Moment::from_nanos(u64::MAX));
    file_lock::lock(context, file.as_handle(), kind, until).map_err(|cause| match cause {
        FileLockError::Timeout => Refused::Timeout,
        FileLockError::Cancelled => Refused::Cancelled,
        FileLockError::Unavailable => Refused::Storage(std::io::Error::other(cause)),
        FileLockError::Io(cause) => Refused::Storage(cause),
    })
}
