// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Synchronous WinHTTP handles stay with one exclusive owner until checked retirement.
use super::{Failure, check, invalid};
use std::{
    io::{self, Read},
    time::Duration,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_update::http::{Endpoint, Headers};
use windows::{
    Win32::Networking::WinHttp::*,
    core::{HSTRING, PCWSTR, w},
};

#[derive(Default)]
pub(super) struct Handles {
    session: usize,
    connection: usize,
    request: usize,
}
impl Handles {
    /// No native handle may remain when the retained memory claim is released.
    pub(super) fn empty(&self) -> bool {
        self.session == 0 && self.connection == 0 && self.request == 0
    }
    /// Close children before parents, retaining every unsuccessful slot for explicit retry.
    pub(super) fn close(&mut self) -> io::Result<()> {
        self.close_using(close)
    }
    /// The external close boundary is replaceable for retained-failure qualification.
    fn close_using(&mut self, close: impl Fn(usize) -> io::Result<()>) -> io::Result<()> {
        for handle in [&mut self.request, &mut self.connection, &mut self.session] {
            if *handle != 0 {
                close(*handle)?;
                *handle = 0;
            }
        }
        Ok(())
    }
}
/// Every call is synchronous; no request callback or native pointer can escape this borrow.
pub(super) fn open(
    handles: &mut Handles,
    endpoint: &Endpoint,
    offset: Option<u64>,
    idle: Duration,
    context: &JobContext,
) -> Result<Headers, Failure> {
    check(context)?;
    if !handles.empty() {
        return Err(invalid("HTTP handles already occupied").into());
    }
    let proxy = if endpoint.secure() {
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
    } else {
        WINHTTP_ACCESS_TYPE_NO_PROXY
    };
    // SAFETY: Static NUL strings, synchronous mode, no callbacks or borrowed request data.
    handles.session = adopt(unsafe {
        WinHttpOpen(w!("SugarSubstitute updater"), proxy, PCWSTR::null(), PCWSTR::null(), 0)
    })?;
    option(handles.session, WINHTTP_OPTION_ENABLE_HTTP_PROTOCOL, 0)?;
    option(handles.session, WINHTTP_OPTION_CONNECT_RETRIES, 1)?;
    // SAFETY: The session owns these two initialized DWORDs for the synchronous option call.
    unsafe {
        WinHttpSetOption(
            Some(raw(handles.session)),
            WINHTTP_OPTION_FAILED_CONNECTION_RETRIES,
            Some(&[0; 8]),
        )
    }
    .map_err(io::Error::other)?;
    timeout(handles.session, idle, context)?;
    let host = HSTRING::from(endpoint.host()?);
    // SAFETY: The owned session and UTF-16 host remain live throughout this synchronous call.
    handles.connection =
        adopt(unsafe { WinHttpConnect(raw(handles.session), &host, endpoint.port()?, 0) })?;
    check(context)?;
    let path = HSTRING::from(endpoint.request_target());
    let mut flags = WINHTTP_FLAG_REFRESH | WINHTTP_FLAG_ESCAPE_DISABLE;
    if endpoint.secure() {
        flags |= WINHTTP_FLAG_SECURE;
    }
    // SAFETY: Exclusive retained connection, escaped bounded path, static verb and no callback.
    handles.request = adopt(unsafe {
        WinHttpOpenRequest(
            raw(handles.connection),
            w!("GET"),
            &path,
            w!("HTTP/1.1"),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        )
    })?;
    option(
        handles.request,
        WINHTTP_OPTION_DISABLE_FEATURE,
        WINHTTP_DISABLE_AUTHENTICATION | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_REDIRECTS,
    )?;
    option(
        handles.request,
        WINHTTP_OPTION_AUTOLOGON_POLICY,
        WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH,
    )?;
    option(handles.request, WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_NEVER)?;
    option(handles.request, WINHTTP_OPTION_MAX_RESPONSE_HEADER_SIZE, 16 * 1024)?;
    option(handles.request, WINHTTP_OPTION_DECOMPRESSION, 0)?;
    let mut headers = "Accept-Encoding: identity\r\n".to_owned();
    if let Some(offset) = offset {
        headers.push_str(&format!("Range: bytes={offset}-\r\n"));
    }
    let headers: Vec<u16> = headers.encode_utf16().collect();
    timeout(handles.request, idle, context)?;
    // SAFETY: All header units remain live until the synchronous GET send finishes; no body.
    unsafe { WinHttpSendRequest(raw(handles.request), Some(&headers), None, 0, 0, 0) }
        .map_err(io::Error::other)?;
    timeout(handles.request, idle, context)?;
    // SAFETY: The retained synchronous request has no asynchronous context.
    unsafe { WinHttpReceiveResponse(raw(handles.request), std::ptr::null_mut()) }
        .map_err(io::Error::other)?;
    check(context)?;
    response(handles.request)
}
/// A body reader cannot outlive the retained native request or its admitted job context.
pub(super) struct Body<'a> {
    handles: &'a Handles,
    idle: Duration,
    context: &'a JobContext,
}
impl<'a> Body<'a> {
    pub(super) fn new(handles: &'a Handles, idle: Duration, context: &'a JobContext) -> Self {
        Self { handles, idle, context }
    }
}
impl Read for Body<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        timeout(self.handles.request, self.idle, self.context)?;
        let length = bytes.len().min(256 * 1024) as u32;
        let mut count = 0;
        // SAFETY: The exclusive synchronous request and complete writable slice outlive this call.
        unsafe {
            WinHttpReadData(
                raw(self.handles.request),
                bytes.as_mut_ptr().cast(),
                length,
                &mut count,
            )
        }
        .map_err(io::Error::other)?;
        check(self.context)?;
        if count > length {
            return Err(invalid("Update HTTP read exceeded buffer"));
        }
        Ok(count as usize)
    }
}
/// Convert positive finite idle and remaining job budgets without passing WinHTTP's infinite zero.
pub(super) fn milliseconds(context: &JobContext, idle: Duration) -> io::Result<i32> {
    check(context)?;
    if idle < Duration::from_millis(1) || idle > Duration::from_secs(120) {
        return Err(invalid("HTTP idle timeout outside supported bounds"));
    }
    let remaining = context
        .deadline()
        .ok_or_else(|| invalid("HTTP deadline absent"))?
        .as_nanos()
        .checked_sub(context.now().as_nanos())
        .ok_or(io::ErrorKind::TimedOut)?
        / 1_000_000;
    let value = remaining.min(idle.as_millis() as u64);
    if value == 0 {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(value as i32)
}
/// Native limits bound each I/O phase; cancellation is checked again when the call returns.
fn timeout(handle: usize, idle: Duration, context: &JobContext) -> io::Result<()> {
    let value = milliseconds(context, idle)?;
    // SAFETY: This retained synchronous handle receives four strictly positive finite limits.
    unsafe { WinHttpSetTimeouts(raw(handle), value, value, value, value) }.map_err(io::Error::other)
}
/// Query into fixed native buffers; oversized headers are errors rather than allocation requests.
fn response(handle: usize) -> Result<Headers, Failure> {
    let mut status = 0u32;
    let mut size = 4;
    let mut index = 0;
    // SAFETY: Status output is one initialized DWORD with exactly four advertised bytes.
    unsafe {
        WinHttpQueryHeaders(
            raw(handle),
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut size,
            &mut index,
        )
    }
    .map_err(io::Error::other)?;
    if size != 4 || status > u16::MAX as u32 {
        return Err(invalid("HTTP status output invalid").into());
    }
    let mut units = [0u16; 8192];
    let mut size = (units.len() * 2) as u32;
    let mut index = 0;
    // SAFETY: The complete initialized UTF-16 array is writable through this synchronous query.
    unsafe {
        WinHttpQueryHeaders(
            raw(handle),
            WINHTTP_QUERY_RAW_HEADERS_CRLF,
            PCWSTR::null(),
            Some(units.as_mut_ptr().cast()),
            &mut size,
            &mut index,
        )
    }
    .map_err(io::Error::other)?;
    if size as usize > units.len() * 2 || !size.is_multiple_of(2) {
        return Err(invalid("HTTP header output invalid").into());
    }
    let text = String::from_utf16(&units[..size as usize / 2]).map_err(io::Error::other)?;
    Ok(Headers::parse(status as u16, text.trim_end_matches('\0'))?)
}
/// Adopt each successful native output immediately, before any subsequent fallible operation.
fn adopt(handle: *mut std::ffi::c_void) -> io::Result<usize> {
    if handle.is_null() { Err(io::Error::last_os_error()) } else { Ok(handle as usize) }
}
/// The integer is private retained custody, not a caller-provided native handle.
fn raw(handle: usize) -> *mut std::ffi::c_void {
    handle as *mut _
}
/// Set one fixed DWORD option while retaining its backing bytes through the native call.
fn option(handle: usize, name: u32, value: u32) -> io::Result<()> {
    // SAFETY: This owned synchronous handle and initialized DWORD outlive the option call.
    unsafe { WinHttpSetOption(Some(raw(handle)), name, Some(&value.to_ne_bytes())) }
        .map_err(io::Error::other)
}
/// Only the retained slot owner invokes native closure, clearing the slot after success.
fn close(handle: usize) -> io::Result<()> {
    // SAFETY: A nonzero slot owns this still-open WinHTTP handle and has no concurrent users.
    unsafe { WinHttpCloseHandle(raw(handle)) }.map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn failed_native_retirement_retains_original_slots_and_never_closes_a_parent_first()
    -> io::Result<()> {
        let mut handles = Handles { request: 3, connection: 2, session: 1 };
        let calls = RefCell::new(Vec::new());
        assert!(
            handles
                .close_using(|handle| {
                    calls.borrow_mut().push(handle);
                    if handle == 2 {
                        Err(io::Error::other("injected close failure"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        assert_eq!(*calls.borrow(), [3, 2]);
        assert_eq!((handles.request, handles.connection, handles.session), (0, 2, 1));
        handles.close_using(|handle| {
            calls.borrow_mut().push(handle);
            Ok(())
        })?;
        assert_eq!(*calls.borrow(), [3, 2, 2, 1]);
        assert!(handles.empty());
        Ok(())
    }
}
