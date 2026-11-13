// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Register one thread-pool wait per object and retire every registration before returning.
//!
//! Safety and lifetime invariants: the boxed `Notify` is the callback context of every
//! registration and stays at a stable address until each `UnregisterWaitEx` with the
//! `INVALID_HANDLE_VALUE` sentinel has returned, which happens only after any running callback
//! has finished. If any unregistration fails the context is leaked rather than freed.

use super::*;
use std::{ffi::c_void, os::windows::io::AsRawHandle};
use tokio::sync::Notify;
use windows::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        INFINITE, RegisterWaitForSingleObject, UnregisterWaitEx, WT_EXECUTEINWAITTHREAD,
        WT_EXECUTEONLYONCE, WaitForSingleObject,
    },
};

fn native(cause: windows::core::Error) -> HandleWaitError {
    HandleWaitError::Io(io::Error::other(cause))
}

fn raw(handle: &BorrowedHandle<'_>) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

/// Lowest index already signalled, observed without waiting.
pub(super) fn signalled(handles: &[BorrowedHandle<'_>]) -> Result<Option<usize>, HandleWaitError> {
    for (index, handle) in handles.iter().enumerate() {
        // SAFETY: the borrowed handle is live for this call; a zero timeout never blocks.
        let status = unsafe { WaitForSingleObject(raw(handle), 0) };
        if status == WAIT_OBJECT_0 {
            return Ok(Some(index));
        }
        if status != WAIT_TIMEOUT {
            return Err(native(windows::core::Error::from_thread()));
        }
    }
    Ok(None)
}

/// Wake the waiting worker from the thread-pool wait callback without any other work.
unsafe extern "system" fn woken(context: *mut c_void, _timed_out: bool) {
    // SAFETY: `wait` keeps this Notify alive until every synchronous unregistration returns.
    unsafe { &*context.cast::<Notify>() }.notify_one();
}

/// Register every object, race the first wake against the deadline, then retire registrations.
pub(super) fn wait(
    context: &JobContext,
    handles: &[BorrowedHandle<'_>],
    until: Moment,
) -> Result<usize, HandleWaitError> {
    let notify = Box::new(Notify::new());
    let mut registrations = Vec::with_capacity(handles.len());
    let mut registered = Ok(());
    for handle in handles {
        let mut registration = HANDLE::default();
        // SAFETY: the boxed Notify stays at a stable address until every registration is
        // unregistered with the blocking sentinel below; the object handle is borrowed live.
        let result = unsafe {
            RegisterWaitForSingleObject(
                &mut registration,
                raw(handle),
                Some(woken),
                Some((&raw const *notify).cast()),
                INFINITE,
                WT_EXECUTEONLYONCE | WT_EXECUTEINWAITTHREAD,
            )
        };
        match result {
            Ok(()) => registrations.push(registration),
            Err(cause) => {
                registered = Err(native(cause));
                break;
            }
        }
    }
    let ended = registered.and_then(|()| race(notify.notified(), until, context));
    let mut retired = Ok(());
    for registration in registrations {
        // SAFETY: the sentinel blocks until any running callback for this registration returns.
        if let Err(cause) = unsafe { UnregisterWaitEx(registration, Some(INVALID_HANDLE_VALUE)) } {
            retired = Err(native(cause));
        }
    }
    if let Err(cause) = retired {
        tracing::error!(
            operation = "execution_handle_wait_abandoned",
            "Object wait context retained without proven unregistration"
        );
        std::mem::forget(notify);
        return Err(cause);
    }
    match ended? {
        Ended::Signalled => signalled(handles)?.ok_or_else(|| {
            HandleWaitError::Io(io::Error::other("Object wait woke without a signalled object"))
        }),
        Ended::Deadline => signalled(handles)?.ok_or(HandleWaitError::Timeout),
        Ended::Cancelled => Err(HandleWaitError::Cancelled),
    }
}
