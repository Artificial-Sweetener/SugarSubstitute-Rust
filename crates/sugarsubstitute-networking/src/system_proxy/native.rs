// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! WinHTTP asynchronous resolver custody; only its bounded callback touches shared state.
mod callback;
mod settings;
use super::{Route, check};
use callback::{State, callback};
use std::{
    ffi::c_void,
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::JobContext;
use windows::{
    Win32::{Foundation::ERROR_IO_PENDING, Networking::WinHttp::*},
    core::{PCWSTR, w},
};

#[derive(Default)]
/// Keep handles, callback state and borrowed strings together until native retirement.
pub(super) struct Resolver {
    session: usize,
    resolver: usize,
    state: Option<Arc<State>>,
    address: Vec<u16>,
    pac: Vec<u16>,
}
impl Resolver {
    /// Only absence of both handles and callback custody permits owner release.
    pub(super) fn settled(&self) -> bool {
        self.session == 0 && self.resolver == 0 && self.state.is_none()
    }
    /// Start asynchronous WinHTTP discovery and observe it on the admitted blocking worker.
    pub(super) fn resolve(
        &mut self,
        address: &str,
        pac: Option<&str>,
        context: &JobContext,
    ) -> io::Result<Vec<Route>> {
        self.address = address.encode_utf16().chain(Some(0)).collect();
        let options = settings::options(&mut self.pac, pac)?;
        // SAFETY: all strings are terminated; handles remain exclusively owned here. Callback
        // context has its own Arc reference, released only by HANDLE_CLOSING, the final callback.
        unsafe {
            self.session = WinHttpOpen(
                w!("SugarSubstitute proxy resolver"),
                WINHTTP_ACCESS_TYPE_NO_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                WINHTTP_FLAG_ASYNC,
            ) as usize;
            if self.session == 0 {
                return Err(io::Error::last_os_error());
            }
            let previous = WinHttpSetStatusCallback(
                self.session as *mut c_void,
                Some(callback),
                WINHTTP_CALLBACK_FLAG_GETPROXYFORURL_COMPLETE
                    | WINHTTP_CALLBACK_FLAG_REQUEST_ERROR
                    | WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING,
                0,
            );
            if previous.is_some_and(|value| value as usize == usize::MAX) {
                return Err(io::Error::last_os_error());
            }
            let mut handle = std::ptr::null_mut();
            status(WinHttpCreateProxyResolver(self.session as *const c_void, &mut handle))?;
            self.resolver = handle as usize;
            let state = Arc::new(State::default());
            let pointer = Arc::into_raw(state.clone()) as usize;
            if let Err(cause) = WinHttpSetOption(
                Some(handle),
                WINHTTP_OPTION_CONTEXT_VALUE,
                Some(&pointer.to_ne_bytes()),
            ) {
                // No API received this context; returning its dedicated reference is safe.
                drop(Arc::from_raw(pointer as *const State));
                return Err(io::Error::other(cause));
            }
            self.state = Some(state.clone());
            let result = WinHttpGetProxyForUrlEx(
                handle,
                PCWSTR(self.address.as_ptr()),
                &options,
                Some(pointer),
            );
            if result != ERROR_IO_PENDING.0 {
                status(result)?;
            }
            let mut observation = state.lock();
            while observation.result.is_none() {
                check(context)?;
                observation = state
                    .changed
                    .wait_timeout(observation, Duration::from_millis(50))
                    .map_err(|_| io::Error::other("Proxy callback synchronization failed"))?
                    .0;
            }
            status(observation.result.ok_or_else(|| io::Error::other("Proxy completion absent"))?)?;
            drop(observation);
            check(context)?;
            let mut result = WINHTTP_PROXY_RESULT::default();
            status(WinHttpGetProxyResult(handle, &mut result))?;
            let routes = settings::routes(&result);
            WinHttpFreeProxyResult(&mut result);
            routes
        }
    }
    /// Close native work and wait for the final callback without extending lookup deadlines.
    pub(super) fn retire(&mut self) -> io::Result<()> {
        close(&mut self.resolver)?;
        close(&mut self.session)?;
        if let Some(state) = &self.state {
            let until = Instant::now() + Duration::from_secs(5);
            let mut observation = state.lock();
            while !observation.closed {
                let remaining =
                    until.checked_duration_since(Instant::now()).ok_or(io::ErrorKind::TimedOut)?;
                observation = state
                    .changed
                    .wait_timeout(observation, remaining)
                    .map_err(|_| io::Error::other("Proxy retirement synchronization failed"))?
                    .0;
            }
        }
        self.state = None;
        self.address.clear();
        self.pac.clear();
        Ok(())
    }
}
impl Drop for Resolver {
    fn drop(&mut self) {
        if !self.settled() {
            // A violated custody contract must not free strings still borrowed by native work.
            // The callback retains its own Arc until the final OS notification.
            std::mem::forget(std::mem::take(&mut self.address));
            std::mem::forget(std::mem::take(&mut self.pac));
        }
    }
}
/// Clear original custody only after the OS accepts the close.
fn close(handle: &mut usize) -> io::Result<()> {
    if *handle != 0 {
        // SAFETY: the original handle is exclusively owned and retained on failure.
        unsafe { WinHttpCloseHandle(*handle as *mut c_void) }.map_err(io::Error::other)?;
        *handle = 0;
    }
    Ok(())
}
/// Preserve WinHTTP's numeric failure cause without including destination secrets.
fn status(code: u32) -> io::Result<()> {
    if code == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(code as i32)) }
}
