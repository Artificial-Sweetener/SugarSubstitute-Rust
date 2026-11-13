// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Final-callback ownership for one asynchronous WinHTTP resolver.
use std::{
    ffi::c_void,
    sync::{Arc, Condvar, Mutex, MutexGuard},
};
use windows::Win32::Networking::WinHttp::*;
#[derive(Default)]
/// The terminal resolution status and independent final-callback acknowledgement.
pub(super) struct Observation {
    pub(super) result: Option<u32>,
    pub(super) closed: bool,
}
#[derive(Default)]
/// Shared state remains alive through the native context's retained Arc reference.
pub(super) struct State {
    value: Mutex<Observation>,
    pub(super) changed: Condvar,
}
impl State {
    /// Recover callback custody after poisoning so native teardown cannot strand its context.
    pub(super) fn lock(&self) -> MutexGuard<'_, Observation> {
        self.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
/// Publish terminal status without running application work on a Windows callback thread.
/// # Safety
/// Nonzero context owns an Arc reference registered on this resolver. Windows must deliver
/// HANDLE_CLOSING exactly once, last; status information must match its documented ABI.
pub(super) unsafe extern "system" fn callback(
    _: *mut c_void,
    context: usize,
    status: u32,
    information: *mut c_void,
    length: u32,
) {
    if context == 0 {
        return;
    }
    // SAFETY: WinHTTP received one retained Arc reference at context registration. Its last
    // callback is HANDLE_CLOSING; only that callback consumes the reference, after unlocking.
    let state = unsafe { &*(context as *const State) };
    {
        let mut value = state.lock();
        match status {
            WINHTTP_CALLBACK_STATUS_GETPROXYFORURL_COMPLETE => value.result = Some(0),
            WINHTTP_CALLBACK_STATUS_REQUEST_ERROR => {
                value.result = Some(
                    if !information.is_null()
                        && length as usize == std::mem::size_of::<WINHTTP_ASYNC_RESULT>()
                    {
                        // SAFETY: WinHTTP supplies this structure for REQUEST_ERROR; checked size.
                        unsafe { (*information.cast::<WINHTTP_ASYNC_RESULT>()).dwError }
                    } else {
                        13
                    },
                );
            }
            WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING => value.closed = true,
            _ => return,
        }
        state.changed.notify_all();
    }
    if status == WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING {
        // SAFETY: consumes exactly the context reference, once, after the final notification.
        drop(unsafe { Arc::from_raw(context as *const State) });
    }
}
