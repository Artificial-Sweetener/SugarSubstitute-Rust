// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own synchronous WinHTTP handles on admitted finite infrastructure workers.
use super::{
    owner::{HttpPort, Transaction, checkpoint},
    probe::Endpoint,
    types::Listener,
    *,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::backend_listener::ConnectedLoopback;
use windows::{
    Win32::Networking::{
        WinHttp::*,
        WinSock::{AF_INET, SOCKADDR_IN, SOCKADDR_STORAGE},
    },
    core::{PCWSTR, w},
};
pub(super) struct SystemPort;
pub(super) mod retry;
#[cfg(test)]
#[path = "native/timeout_tests.rs"]
mod timeout_tests;
impl HttpPort for SystemPort {
    fn begin(
        &self,
        state: &mut Transaction,
        port: u16,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError> {
        checkpoint(context)?;
        if state.peer.is_none()
            && !sugarsubstitute_supervision::backend_listener::loopback_listener_present(
                port,
                &mut state.tcp,
                context,
            )
            .map_err(|cause| ReadinessTransportError::new("http_listener_query", cause))?
        {
            return Err(ReadinessTransportError::unavailable(
                "http_listener_absent",
                std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
            ));
        }
        checkpoint(context)?;
        // SAFETY: Literal user agent and explicit NO_PROXY disable proxy discovery; synchronous session has no callback.
        let handle = unsafe {
            WinHttpOpen(
                w!("SugarSubstitute readiness"),
                WINHTTP_ACCESS_TYPE_NO_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            )
        };
        state.session = adopt(handle, "http_session")?;
        checkpoint(context)?;
        option(state.session, WINHTTP_OPTION_ENABLE_HTTP_PROTOCOL, 0, context)?;
        option(state.session, WINHTTP_OPTION_CONNECT_RETRIES, 1, context)?;
        checkpoint(context)?;
        // SAFETY: Two initialized DWORDs encode zero automatic failed-connection retries and no retry conditions.
        // The private session is configured before creating any connection or request.
        unsafe {
            WinHttpSetOption(
                Some(raw(state.session)),
                WINHTTP_OPTION_FAILED_CONNECTION_RETRIES,
                Some(&[0; 8]),
            )
        }
        .map_err(|e| ReadinessTransportError::new("http_disable_failed_connection_retries", e))?;
        checkpoint(context)?;
        timeout(state.session, context)?;
        checkpoint(context)?;
        // SAFETY: The retained session and exact literal IPv4 loopback name live through connection construction.
        let handle = unsafe { WinHttpConnect(raw(state.session), w!("127.0.0.1"), port, 0) };
        state.connect = adopt(handle, "http_connect")?;
        checkpoint(context)
    }
    fn get(
        &self,
        state: &mut Transaction,
        input: &ReadinessProbeInput,
        endpoint: Endpoint<'_>,
        context: &JobContext,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError> {
        checkpoint(context)?;
        if state.request != 0 {
            return Err(ReadinessTransportError::invalid("http_previous_request_retained"));
        }
        let mut path: Vec<u16> = endpoint.path().encode_utf16().collect();
        path.push(0);
        // SAFETY: Only the fixed endpoint owner supplies this bounded NUL path. No referrer or custom accept list is supplied.
        let handle = unsafe {
            WinHttpOpenRequest(
                raw(state.connect),
                w!("GET"),
                PCWSTR(path.as_ptr()),
                w!("HTTP/1.1"),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_REFRESH | WINHTTP_FLAG_ESCAPE_DISABLE,
            )
        };
        state.request = adopt(handle, "http_request")?;
        checkpoint(context)?;
        option(
            state.request,
            WINHTTP_OPTION_DISABLE_FEATURE,
            WINHTTP_DISABLE_AUTHENTICATION | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_REDIRECTS,
            context,
        )?;
        option(
            state.request,
            WINHTTP_OPTION_REDIRECT_POLICY,
            WINHTTP_OPTION_REDIRECT_POLICY_NEVER,
            context,
        )?;
        option(state.request, WINHTTP_OPTION_MAX_RESPONSE_HEADER_SIZE, 16 * 1024, context)?;
        option(state.request, WINHTTP_OPTION_DECOMPRESSION, 0, context)?;
        timeout(state.request, context)?;
        // SAFETY: Retained synchronous request; GET sends no body, extra headers, credentials or callback context.
        let sent = unsafe { WinHttpSendRequest(raw(state.request), None, None, 0, 0, 0) };
        retry::complete(sent, "http_send", state.peer.is_some(), context)?;
        timeout(state.request, context)?;
        // SAFETY: Synchronous owned request has no asynchronous reserved context.
        let received = unsafe { WinHttpReceiveResponse(raw(state.request), std::ptr::null_mut()) };
        retry::complete(received, "http_receive", state.peer.is_some(), context)?;
        let before = connection(state.request, input.port(), context)?;
        qualify(state, input, before, context)?;
        let declared = headers(state.request, endpoint, context)?;
        let mut scratch = [0u8; 32 * 1024];
        let mut total = 0u64;
        loop {
            timeout(state.request, context)?;
            let mut read = 0;
            // SAFETY: Complete fixed scratch slice and initialized count live until this synchronous read returns.
            let received = unsafe {
                WinHttpReadData(
                    raw(state.request),
                    scratch.as_mut_ptr().cast(),
                    scratch.len() as u32,
                    &mut read,
                )
            };
            retry::complete(received, "http_body_read", state.peer.is_some(), context)?;
            if read as usize > scratch.len() {
                return Err(ReadinessTransportError::invalid("http_read_count"));
            }
            if read == 0 {
                break;
            }
            total = total
                .checked_add(u64::from(read))
                .filter(|n| *n <= endpoint.maximum())
                .ok_or_else(|| ReadinessTransportError::invalid("http_body_bound"))?;
            consume(&scratch[..read as usize])?;
        }
        if declared.is_some_and(|length| length != total) {
            return Err(ReadinessTransportError::invalid("http_truncated_or_excess_body"));
        }
        let after = connection(state.request, input.port(), context)?;
        if after != before {
            return Err(ReadinessTransportError::invalid("http_connected_tuple_changed"));
        }
        qualify(state, input, after, context)?;
        close_slot(&mut state.request, context)
    }
    fn close(
        &self,
        state: &mut Transaction,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError> {
        close_slot(&mut state.request, context)?;
        close_slot(&mut state.connect, context)?;
        close_slot(&mut state.session, context)
    }
}
fn raw(value: usize) -> *mut core::ffi::c_void {
    value as *mut _
}
fn adopt(
    value: *mut core::ffi::c_void,
    stage: &'static str,
) -> Result<usize, ReadinessTransportError> {
    if value.is_null() {
        Err(ReadinessTransportError::new(stage, std::io::Error::last_os_error()))
    } else {
        Ok(value as usize)
    }
}
fn option(
    handle: usize,
    key: u32,
    value: u32,
    context: &JobContext,
) -> Result<(), ReadinessTransportError> {
    checkpoint(context)?;
    // SAFETY: Retained handle and initialized DWORD bytes live throughout the synchronous option call.
    unsafe { WinHttpSetOption(Some(raw(handle)), key, Some(&value.to_ne_bytes())) }
        .map_err(|e| ReadinessTransportError::new("http_option", e))?;
    checkpoint(context)
}
fn timeout(handle: usize, context: &JobContext) -> Result<(), ReadinessTransportError> {
    checkpoint(context)?;
    let milliseconds = timeout_milliseconds(context.deadline(), context.now())?;
    // SAFETY: Retained synchronous handle; positive timeout bounds each later native I/O phase.
    unsafe {
        WinHttpSetTimeouts(raw(handle), milliseconds, milliseconds, milliseconds, milliseconds)
    }
    .map_err(|e| ReadinessTransportError::new("http_timeouts", e))?;
    checkpoint(context)
}
/// Floor the remaining original job budget to positive finite WinHTTP milliseconds.
/// Consume a valid submillisecond remainder as elapsed rather than rounding beyond
/// the deadline or passing WinHTTP's infinite zero. Missing/past deadlines reject.
pub(super) fn timeout_milliseconds(
    deadline: Option<sugarsubstitute_contracts::execution::Moment>,
    now: sugarsubstitute_contracts::execution::Moment,
) -> Result<i32, ReadinessTransportError> {
    let remaining = deadline
        .ok_or_else(|| ReadinessTransportError::invalid("http_deadline"))?
        .as_nanos()
        .checked_sub(now.as_nanos())
        .ok_or_else(|| ReadinessTransportError::invalid("http_deadline"))?
        / 1_000_000;
    if remaining == 0 {
        return Err(ReadinessTransportError::elapsed(
            "http_submillisecond_deadline",
            std::io::Error::from(std::io::ErrorKind::TimedOut),
        ));
    }
    Ok(remaining.min(i32::MAX as u64) as i32)
}
fn close_slot(slot: &mut usize, context: &JobContext) -> Result<(), ReadinessTransportError> {
    if *slot != 0 {
        checkpoint(context)?;
        // SAFETY: This unique retained slot owns the handle and clears it only after successful closure.
        unsafe { WinHttpCloseHandle(raw(*slot)) }
            .map_err(|e| ReadinessTransportError::new("http_close", e))?;
        *slot = 0;
        checkpoint(context)?;
    }
    Ok(())
}
fn connection(
    handle: usize,
    port: u16,
    context: &JobContext,
) -> Result<ConnectedLoopback, ReadinessTransportError> {
    checkpoint(context)?;
    let mut information = WINHTTP_CONNECTION_INFO {
        cbSize: size_of::<WINHTTP_CONNECTION_INFO>() as u32,
        ..Default::default()
    };
    let mut length = size_of::<WINHTTP_CONNECTION_INFO>() as u32;
    // SAFETY: The entire initialized native connection-info structure is advertised to the retained request.
    unsafe {
        WinHttpQueryOption(
            raw(handle),
            WINHTTP_OPTION_CONNECTION_INFO,
            Some((&mut information as *mut WINHTTP_CONNECTION_INFO).cast()),
            &mut length,
        )
    }
    .map_err(|e| ReadinessTransportError::new("http_connection_information", e))?;
    checkpoint(context)?;
    if length as usize != size_of::<WINHTTP_CONNECTION_INFO>() {
        return Err(ReadinessTransportError::invalid("http_connection_information_length"));
    }
    let local = ipv4(&information.LocalAddress)?;
    let remote = ipv4(&information.RemoteAddress)?;
    if remote != port {
        return Err(ReadinessTransportError::invalid("http_remote_port"));
    }
    ConnectedLoopback::new(local, remote)
        .map_err(|e| ReadinessTransportError::new("http_connected_tuple", e))
}
fn ipv4(address: &SOCKADDR_STORAGE) -> Result<u16, ReadinessTransportError> {
    if address.ss_family != AF_INET {
        return Err(ReadinessTransportError::invalid("http_address_family"));
    }
    // SAFETY: AF_INET establishes SOCKADDR_IN layout; sockaddr storage has sufficient alignment and size.
    let address = unsafe { &*(address as *const SOCKADDR_STORAGE).cast::<SOCKADDR_IN>() };
    // SAFETY: The IPv4 address union's S_addr field holds the complete network-order address bytes.
    if unsafe { address.sin_addr.S_un.S_addr }.to_ne_bytes() != [127, 0, 0, 1] {
        return Err(ReadinessTransportError::invalid("http_non_loopback_peer"));
    }
    Ok(u16::from_be(address.sin_port))
}
fn qualify(
    state: &mut Transaction,
    input: &ReadinessProbeInput,
    connection: ConnectedLoopback,
    context: &JobContext,
) -> Result<(), ReadinessTransportError> {
    let peer = match &input.listener {
        Listener::Managed(lease, _) => {
            lease.qualify_connection(connection, &mut state.tcp, context)
        }
        Listener::Attached(lease, _) => {
            lease.qualify_connection(connection, &mut state.tcp, context)
        }
    }
    .map_err(|e| ReadinessTransportError::new("http_native_peer", e))?;
    if state.peer.is_some_and(|old| old != peer) {
        return Err(ReadinessTransportError::invalid("http_peer_generation_changed"));
    }
    state.peer = Some(peer);
    Ok(())
}
fn headers(
    handle: usize,
    endpoint: Endpoint<'_>,
    context: &JobContext,
) -> Result<Option<u64>, ReadinessTransportError> {
    checkpoint(context)?;
    let mut status = 0u32;
    let mut length = 4;
    let mut index = 0;
    // SAFETY: Numeric status uses one initialized DWORD of exactly four advertised bytes.
    unsafe {
        WinHttpQueryHeaders(
            raw(handle),
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut length,
            &mut index,
        )
    }
    .map_err(|e| ReadinessTransportError::new("http_status", e))?;
    checkpoint(context)?;
    if status != 200 || length != 4 {
        return Err(ReadinessTransportError::invalid("http_status_not_200"));
    }
    let mut units = [0u16; 8192];
    let mut length = (units.len() * 2) as u32;
    let mut index = 0;
    // SAFETY: Bounded complete UTF-16 header storage is advertised; overflow is an error with no retry allocation.
    unsafe {
        WinHttpQueryHeaders(
            raw(handle),
            WINHTTP_QUERY_RAW_HEADERS_CRLF,
            PCWSTR::null(),
            Some(units.as_mut_ptr().cast()),
            &mut length,
            &mut index,
        )
    }
    .map_err(|e| ReadinessTransportError::new("http_headers", e))?;
    checkpoint(context)?;
    if length as usize > units.len() * 2 || !length.is_multiple_of(2) {
        return Err(ReadinessTransportError::invalid("http_headers_bound"));
    }
    let count = length as usize / 2;
    let text = String::from_utf16(&units[..count])
        .map_err(|e| ReadinessTransportError::new("http_headers_unicode", e))?;
    super::headers::parse_headers(text.trim_end_matches('\0'), endpoint)
}
