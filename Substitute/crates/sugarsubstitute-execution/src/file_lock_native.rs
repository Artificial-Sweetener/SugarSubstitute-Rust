// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Issue one overlapped `LockFileEx` and retire it before returning.
//!
//! Safety and lifetime invariants: the boxed `OVERLAPPED` and its manual-reset event are owned by
//! one call and are released only after the kernel request is proven complete, either by its
//! completion or by `GetOverlappedResult` waiting after `CancelIoEx`. The registered wait's
//! callback context outlives `UnregisterWaitEx(INVALID_HANDLE_VALUE)`, which returns only after
//! any running callback has finished. When completion cannot be proven, the allocations are
//! leaked rather than freed under the kernel.

use super::*;
use std::{ffi::c_void, os::windows::io::AsRawHandle};
use tokio::sync::Notify;
use windows::{
    Wdk::Storage::FileSystem::{
        FILE_MODE_INFORMATION, FileModeInformation, NtQueryInformationFile,
    },
    Win32::{
        Foundation::{
            CloseHandle, ERROR_IO_PENDING, ERROR_NOT_FOUND, ERROR_OPERATION_ABORTED, HANDLE,
            INVALID_HANDLE_VALUE,
        },
        Storage::FileSystem::{LOCK_FILE_FLAGS, LOCKFILE_EXCLUSIVE_LOCK, LockFileEx, UnlockFileEx},
        System::{
            IO::{CancelIoEx, GetOverlappedResult, IO_STATUS_BLOCK, OVERLAPPED},
            Threading::{
                CreateEventW, INFINITE, RegisterWaitForSingleObject, UnregisterWaitEx,
                WT_EXECUTEINWAITTHREAD, WT_EXECUTEONLYONCE,
            },
        },
    },
};

/// Kernel mode bits that make every I/O on a handle synchronous.
const SYNCHRONOUS_IO: u32 = 0x10 | 0x20;

/// Map the requested mode onto `LockFileEx` flags; a wait never sets fail-immediately.
fn flags(kind: LockKind) -> LOCK_FILE_FLAGS {
    match kind {
        LockKind::Shared => LOCK_FILE_FLAGS(0),
        LockKind::Exclusive => LOCKFILE_EXCLUSIVE_LOCK,
    }
}

fn native(cause: windows::core::Error) -> FileLockError {
    FileLockError::Io(io::Error::other(cause))
}

/// One kernel request's storage, kept at a stable address for the request's whole lifetime.
struct Request {
    overlapped: Box<OVERLAPPED>,
    event: HANDLE,
}
impl Request {
    fn new() -> Result<Self, FileLockError> {
        // SAFETY: a private unnamed manual-reset event; this owner closes it exactly once.
        let event = unsafe { CreateEventW(None, true, false, None) }.map_err(native)?;
        Ok(Self { overlapped: Box::new(OVERLAPPED { hEvent: event, ..Default::default() }), event })
    }
    /// Keep the storage alive for the process when kernel completion could not be proven.
    fn abandon(self) {
        tracing::error!(
            operation = "execution_file_lock_abandoned",
            "Outstanding lock request storage retained without proven completion"
        );
        std::mem::forget(self);
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        // SAFETY: the event was created by `new` and no request references it any more.
        if let Err(cause) = unsafe { CloseHandle(self.event) } {
            tracing::error!(
                operation = "execution_file_lock_event_close",
                %cause,
                "Lock wait event close failed"
            );
        }
    }
}

/// Reject handles whose I/O is synchronous: `LockFileEx` would block without any deadline.
fn require_overlapped(handle: HANDLE) -> Result<(), FileLockError> {
    let mut mode = FILE_MODE_INFORMATION::default();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: the borrowed handle is live for this call; both outputs are owned locals.
    unsafe {
        NtQueryInformationFile(
            handle,
            &mut status,
            (&raw mut mode).cast(),
            size_of::<FILE_MODE_INFORMATION>() as u32,
            FileModeInformation,
        )
    }
    .ok()
    .map_err(native)?;
    if mode.Mode & SYNCHRONOUS_IO != 0 {
        return Err(FileLockError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "File lock waits require a handle opened for overlapped I/O",
        )));
    }
    Ok(())
}

/// Wake the waiting worker from the thread-pool wait callback without any other work.
unsafe extern "system" fn signalled(context: *mut c_void, _timed_out: bool) {
    // SAFETY: `wait_for_event` keeps this Notify alive until synchronous unregistration returns.
    unsafe { &*context.cast::<Notify>() }.notify_one();
}

/// Race the request's completion event against the deadline and cancellation.
fn wait_for_event(
    request: &Request,
    until: Moment,
    context: &JobContext,
) -> Result<Ended, FileLockError> {
    let notify = Box::new(Notify::new());
    let mut registration = HANDLE::default();
    // SAFETY: the boxed Notify stays at a stable address until UnregisterWaitEx with the
    // INVALID_HANDLE_VALUE sentinel has returned below, so no callback observes freed storage.
    unsafe {
        RegisterWaitForSingleObject(
            &mut registration,
            request.event,
            Some(signalled),
            Some((&raw const *notify).cast()),
            INFINITE,
            WT_EXECUTEONLYONCE | WT_EXECUTEINWAITTHREAD,
        )
    }
    .map_err(native)?;
    let ended = race(notify.notified(), until, context);
    // SAFETY: the sentinel blocks until any running callback has returned.
    if let Err(cause) = unsafe { UnregisterWaitEx(registration, Some(INVALID_HANDLE_VALUE)) } {
        std::mem::forget(notify);
        return Err(native(cause));
    }
    ended
}

/// Issue the request, wait, and on any non-granted end cancel it and observe its completion.
pub(super) fn lock(
    context: &JobContext,
    file: BorrowedHandle<'_>,
    kind: LockKind,
    until: Moment,
) -> Result<(), FileLockError> {
    let handle = HANDLE(file.as_raw_handle());
    require_overlapped(handle)?;
    let mut request = Request::new()?;
    // SAFETY: the boxed OVERLAPPED and its event outlive the request: every path below either
    // observes completion or abandons the storage.
    match unsafe {
        LockFileEx(handle, flags(kind), None, u32::MAX, u32::MAX, request.overlapped.as_mut())
    } {
        Ok(()) => return Ok(()),
        Err(cause) if cause.code() == ERROR_IO_PENDING.to_hresult() => {}
        Err(cause) => return Err(native(cause)),
    }
    let ended = wait_for_event(&request, until, context);
    let mut transferred = 0;
    if matches!(ended, Ok(Ended::Granted)) {
        // SAFETY: the event is signalled, so the request completed; its storage is still owned.
        return unsafe {
            GetOverlappedResult(handle, request.overlapped.as_ref(), &mut transferred, false)
        }
        .map_err(native);
    }
    // SAFETY: cancels only this request, identified by its still-owned OVERLAPPED.
    if let Err(cause) = unsafe { CancelIoEx(handle, Some(request.overlapped.as_ref())) }
        && cause.code() != ERROR_NOT_FOUND.to_hresult()
    {
        request.abandon();
        return Err(native(cause));
    }
    // SAFETY: waits for the cancelled request to complete before its storage can be freed.
    match unsafe {
        GetOverlappedResult(handle, request.overlapped.as_ref(), &mut transferred, true)
    } {
        Ok(()) => {
            // Granted while the wait was ending: release it so an error never leaves a lock.
            let mut unlock = OVERLAPPED::default();
            // SAFETY: unlocks the range this call just acquired, with an owned local OVERLAPPED.
            unsafe { UnlockFileEx(handle, None, u32::MAX, u32::MAX, &mut unlock) }
                .map_err(native)?;
        }
        Err(cause) if cause.code() == ERROR_OPERATION_ABORTED.to_hresult() => {}
        Err(cause) => return Err(native(cause)),
    }
    Err(match ended? {
        Ended::Cancelled => FileLockError::Cancelled,
        Ended::Deadline | Ended::Granted => FileLockError::Timeout,
    })
}
