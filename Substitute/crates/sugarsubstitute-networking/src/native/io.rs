// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fixed native buffers, positive phase deadlines and exact loopback tuples.
use super::{Transport, raw};
use crate::{Cause, Failure};
use sugarsubstitute_contracts::execution::JobContext;
use windows::{
    Win32::Networking::{
        WinHttp::*,
        WinSock::{AF_INET, SOCKADDR_IN, SOCKADDR_STORAGE},
    },
    core::PCWSTR,
};

/// Bounded raw response facts; feature parsers decide framing and allowed status codes.
pub struct Response {
    /// Numeric native HTTP status.
    pub status: u16,
    /// At most 8192 UTF-16 units converted to UTF-8, without terminal NULs.
    pub headers: String,
}
impl Transport {
    /// Read at most 256 KiB into the caller's already admitted buffer.
    /// # Errors
    /// Rejects absent requests, cancellation, elapsed deadlines and impossible native counts.
    pub fn read(&mut self, bytes: &mut [u8], context: &JobContext) -> Result<usize, Failure> {
        self.active(context)?;
        check(context)?;
        if bytes.is_empty() {
            return Ok(0);
        }
        timeout(self.request, context, self.idle)?;
        let length = bytes.len().min(256 * 1024) as u32;
        let mut count = 0;
        // SAFETY: Complete caller-owned writable slice and initialized count outlive the synchronous call.
        let result = unsafe {
            WinHttpReadData(raw(self.request), bytes.as_mut_ptr().cast(), length, &mut count)
        };
        check(context)?;
        result.map_err(|e| Failure::native("http_body_read", e))?;
        if count > length {
            return Err(Failure::invalid("http_read_count"));
        }
        Ok(count as usize)
    }
    /// Query status and headers into fixed 16 KiB native storage without resize retries.
    /// # Errors
    /// Rejects impossible lengths, invalid UTF-16, cancellation and native query failures.
    pub fn response(&mut self, context: &JobContext) -> Result<Response, Failure> {
        self.active(context)?;
        check(context)?;
        let mut status = 0u32;
        let mut length = 4;
        let mut index = 0;
        // SAFETY: Status output is one initialized DWORD of exactly four advertised bytes.
        unsafe {
            WinHttpQueryHeaders(
                raw(self.request),
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&mut status as *mut u32).cast()),
                &mut length,
                &mut index,
            )
        }
        .map_err(|e| Failure::native("http_status", e))?;
        check(context)?;
        if length != 4 || status > u16::MAX as u32 {
            return Err(Failure::invalid("http_status_output"));
        }
        let mut units = [0u16; 8192];
        let mut length = (units.len() * 2) as u32;
        let mut index = 0;
        // SAFETY: Complete fixed UTF-16 buffer is advertised; oversized output fails without allocation retry.
        unsafe {
            WinHttpQueryHeaders(
                raw(self.request),
                WINHTTP_QUERY_RAW_HEADERS_CRLF,
                PCWSTR::null(),
                Some(units.as_mut_ptr().cast()),
                &mut length,
                &mut index,
            )
        }
        .map_err(|e| Failure::native("http_headers", e))?;
        check(context)?;
        if length as usize > units.len() * 2 || !length.is_multiple_of(2) {
            return Err(Failure::invalid("http_headers_bound"));
        }
        let mut headers =
            String::from_utf16(&units[..length as usize / 2]).map_err(|cause| Failure {
                stage: "http_headers_unicode",
                cause: Cause::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, cause)),
            })?;
        headers.truncate(headers.trim_end_matches('\0').len());
        Ok(Response { status: status as u16, headers })
    }
    /// Observe exact IPv4 loopback local/remote ports of this connected request.
    /// The feature must qualify the kernel listener/process before and after body consumption.
    /// # Errors
    /// Rejects non-loopback addresses, changed remote port, invalid structures and cancellation.
    pub fn loopback_tuple(
        &mut self,
        port: u16,
        context: &JobContext,
    ) -> Result<(u16, u16), Failure> {
        self.active(context)?;
        check(context)?;
        let mut information = WINHTTP_CONNECTION_INFO {
            cbSize: size_of::<WINHTTP_CONNECTION_INFO>() as u32,
            ..Default::default()
        };
        let mut length = size_of::<WINHTTP_CONNECTION_INFO>() as u32;
        // SAFETY: Entire initialized connection-info structure is writable for the synchronous query.
        unsafe {
            WinHttpQueryOption(
                raw(self.request),
                WINHTTP_OPTION_CONNECTION_INFO,
                Some((&mut information as *mut WINHTTP_CONNECTION_INFO).cast()),
                &mut length,
            )
        }
        .map_err(|e| Failure::native("http_connection_information", e))?;
        check(context)?;
        if length as usize != size_of::<WINHTTP_CONNECTION_INFO>() {
            return Err(Failure::invalid("http_connection_information_length"));
        }
        let local = ipv4(&information.LocalAddress)?;
        let remote = ipv4(&information.RemoteAddress)?;
        if remote != port {
            return Err(Failure::invalid("http_remote_port"));
        }
        Ok((local, remote))
    }
}
/// Decode loopback ports only after verifying the native IPv4 layout and exact address bytes.
fn ipv4(address: &SOCKADDR_STORAGE) -> Result<u16, Failure> {
    if address.ss_family != AF_INET {
        return Err(Failure::invalid("http_address_family"));
    }
    // SAFETY: AF_INET establishes SOCKADDR_IN layout within sufficiently aligned and sized storage.
    let address = unsafe { &*(address as *const SOCKADDR_STORAGE).cast::<SOCKADDR_IN>() };
    // SAFETY: The complete IPv4 address is stored in the S_addr union field.
    if unsafe { address.sin_addr.S_un.S_addr }.to_ne_bytes() != [127, 0, 0, 1] {
        return Err(Failure::invalid("http_non_loopback_peer"));
    }
    Ok(u16::from_be(address.sin_port))
}
/// Observe the original job cutoff and cancellation without extending its lifetime.
pub(super) fn check(context: &JobContext) -> Result<(), Failure> {
    let kind = if context.cancellation_requested() {
        Some(std::io::ErrorKind::Interrupted)
    } else if context.deadline().is_none_or(|until| context.now() >= until) {
        Some(std::io::ErrorKind::TimedOut)
    } else {
        None
    };
    match kind {
        Some(kind) => Err(Failure { stage: "http_checkpoint", cause: Cause::Io(kind.into()) }),
        None => Ok(()),
    }
}
/// Apply strictly positive finite phase limits capped by the immutable route idle policy.
pub(super) fn timeout(
    handle: usize,
    context: &JobContext,
    idle: Option<u32>,
) -> Result<(), Failure> {
    check(context)?;
    if idle.is_some_and(|value| !(1..=120_000).contains(&value)) {
        return Err(Failure::invalid("http_idle_bound"));
    }
    let milliseconds =
        crate::milliseconds(context.deadline(), context.now(), idle).map_err(|cause| Failure {
            stage: if cause == crate::DeadlineError::Elapsed {
                "http_submillisecond_deadline"
            } else {
                "http_deadline"
            },
            cause: Cause::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, cause)),
        })?;
    // SAFETY: Retained synchronous handle receives positive finite limits for all four native phases.
    unsafe {
        WinHttpSetTimeouts(raw(handle), milliseconds, milliseconds, milliseconds, milliseconds)
    }
    .map_err(|e| Failure::native("http_timeouts", e))?;
    check(context)
}
