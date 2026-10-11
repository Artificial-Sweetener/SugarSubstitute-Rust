// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep registered callback storage alive until synchronous unregister proves quiescence.

use crate::native::{NativeObservationError, Signal};
use ::windows::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::Threading::{
        INFINITE, RegisterWaitForSingleObject, UnregisterWaitEx, WT_EXECUTEINWAITTHREAD,
        WT_EXECUTEONLYONCE,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct Callback {
    signal: Arc<Signal>,
    timed_out: AtomicBool,
}
pub(super) struct Wait {
    handle: Option<usize>,
    callback: Box<Callback>,
    uncertain_registration: bool,
}
impl Wait {
    pub(super) fn new(signal: Arc<Signal>) -> Self {
        Self {
            handle: None,
            callback: Box::new(Callback { signal, timed_out: AtomicBool::new(false) }),
            uncertain_registration: false,
        }
    }
    pub(super) fn register(&mut self, process: HANDLE) -> Result<(), NativeObservationError> {
        let mut handle = HANDLE::default();
        let context = self.callback.as_ref() as *const Callback;
        // SAFETY: callback Box has a stable address before registration, including
        // synchronous/reentrant delivery. Its owner retains both it and process
        // until UnregisterWaitEx(INVALID_HANDLE_VALUE) succeeds on another thread.
        let result = unsafe {
            RegisterWaitForSingleObject(
                &mut handle,
                process,
                Some(callback),
                Some(context.cast()),
                INFINITE,
                WT_EXECUTEONLYONCE | WT_EXECUTEINWAITTHREAD,
            )
        };
        if !handle.is_invalid() {
            self.handle = Some(handle.0 as usize);
        }
        result.map_err(|cause| NativeObservationError::new("register_process_wait", cause))?;
        if self.handle.is_none() {
            self.uncertain_registration = true;
            return Err(NativeObservationError::new(
                "register_process_wait",
                std::io::Error::other("Successful registration returned no wait identity"),
            ));
        }
        Ok(())
    }
    pub(super) fn timed_out(&self) -> bool {
        self.callback.timed_out.load(Ordering::Acquire)
    }
    pub(super) fn retire(&mut self) -> Result<(), NativeObservationError> {
        if self.uncertain_registration {
            return Err(NativeObservationError::new(
                "unregister_process_wait",
                std::io::Error::other(
                    "Successful native registration has no retirement identity; callback lifetime remains unresolved",
                ),
            ));
        }
        if let Some(handle) = self.handle {
            // SAFETY: this is the unique registration owner, never its callback
            // thread. The sentinel makes return wait for actual callback completion.
            // On failure identity and callback storage stay retained for retry.
            unsafe { UnregisterWaitEx(HANDLE(handle as *mut _), Some(INVALID_HANDLE_VALUE)) }
                .map_err(|cause| NativeObservationError::new("unregister_process_wait", cause))?;
            self.handle = None;
        }
        Ok(())
    }
}
unsafe extern "system" fn callback(context: *mut std::ffi::c_void, timed_out: bool) {
    // SAFETY: register supplies this stable Callback pointer and synchronous
    // unregister prevents its reclamation until this invocation has returned.
    let callback = unsafe { &*context.cast::<Callback>() };
    callback.timed_out.store(timed_out, Ordering::Release);
    // Only owned atomics and private scheduler wake; no application callback,
    // Core mutex, native handle cleanup, allocation or user clock in this ABI body.
    callback.signal.complete();
}
