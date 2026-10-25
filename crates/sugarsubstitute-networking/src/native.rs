// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exclusive synchronous WinHTTP custody; handles never cross the adapter boundary.
use crate::Failure;
use crate::UpdateRequest;
use sugarsubstitute_contracts::execution::{ExecutionId, JobContext, JobId, Moment, ScopeId};
use windows::{
    Win32::Networking::WinHttp::*,
    core::{HSTRING, PCWSTR, w},
};
mod io;
pub use io::Response;

/// Exclusive retained HTTP handles. Drop does not assert native release.
/// The feature owner must retain its memory claim until `empty()` after `close()`.
#[derive(Default)]
pub struct Transport {
    session: usize,
    connection: usize,
    request: usize,
    idle: Option<u32>,
    context: Option<ContextIdentity>,
}
/// Bind active native effects to the original admitted job and its immutable effective cutoff.
#[derive(Clone, Copy, Eq, PartialEq)]
struct ContextIdentity {
    owner: ExecutionId,
    scope: ScopeId,
    job: JobId,
    deadline: Moment,
}
impl ContextIdentity {
    /// Capture only scheduler-neutral identities; no scheduler handle or cancellation escape is retained.
    fn new(context: &JobContext) -> Result<Self, Failure> {
        Ok(Self {
            owner: context.owner_id(),
            scope: context.scope_id(),
            job: context.job_id(),
            deadline: context.deadline().ok_or_else(|| Failure::invalid("http_deadline"))?,
        })
    }
}
impl Transport {
    /// No native handle remains; this says nothing about feature body memory.
    pub fn empty(&self) -> bool {
        self.session == 0 && self.connection == 0 && self.request == 0
    }
    /// Open fixed IPv4 loopback readiness with proxy discovery disabled.
    /// # Errors
    /// Rejects zero ports, occupied custody, cancellation and native setup failures.
    pub fn readiness(&mut self, port: u16, context: &JobContext) -> Result<(), Failure> {
        if port == 0 {
            return Err(Failure::invalid("http_loopback_port"));
        }
        self.begin("127.0.0.1", port, false, context, None)
    }
    /// Open only a feature-validated update endpoint. TLS keeps native certificate validation;
    /// HTTPS uses automatic proxy selection and fixture HTTP uses no proxy.
    /// `idle` is a per-phase cap in milliseconds (1..=120000); the original job cutoff also applies.
    /// All active operations must use the same admitted job context. Checked retirement may use
    /// a separate cleanup worker after the original job is cancelled or expired.
    /// # Errors
    /// Rejects occupied custody, invalid idle bounds, cancellation and native setup failures.
    /// The caller authorizes the request destination independently before native effects.
    pub fn update(
        &mut self,
        request: &UpdateRequest<'_>,
        idle: u32,
        context: &JobContext,
    ) -> Result<(), Failure> {
        if !(1..=120_000).contains(&idle) {
            return Err(Failure::invalid("http_idle_bound"));
        }
        self.begin(request.host, request.port, request.secure, context, Some(idle))?;
        self.request(request.target, request.secure, context, Some(idle))
    }
    /// Configure one readiness GET path from the independently pinned installed manifest.
    /// # Errors
    /// Rejects non-origin-form/control-bearing paths, unresolved requests and native failures.
    pub fn readiness_request(&mut self, path: &str, context: &JobContext) -> Result<(), Failure> {
        if self.idle.is_some() || path.starts_with("//") {
            return Err(Failure::invalid("http_wrong_route"));
        }
        self.validate_context(context)?;
        self.request(path, false, context, None)
    }
    /// Open one finite synchronous session and retain partial construction before any next effect.
    fn begin(
        &mut self,
        host: &str,
        port: u16,
        proxy: bool,
        context: &JobContext,
        idle: Option<u32>,
    ) -> Result<(), Failure> {
        io::check(context)?;
        if !self.empty() {
            return Err(Failure::invalid("http_handles_occupied"));
        }
        self.context = Some(ContextIdentity::new(context)?);
        self.idle = idle;
        let access =
            if proxy { WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY } else { WINHTTP_ACCESS_TYPE_NO_PROXY };
        let agent = if idle.is_some() {
            w!("SugarSubstitute updater")
        } else {
            w!("SugarSubstitute readiness")
        };
        // SAFETY: Static NUL strings and synchronous mode; no callbacks or borrowed native data.
        self.session = adopt(
            unsafe { WinHttpOpen(agent, access, PCWSTR::null(), PCWSTR::null(), 0) },
            "http_session",
        )?;
        option(self.session, WINHTTP_OPTION_ENABLE_HTTP_PROTOCOL, 0, context)?;
        option(self.session, WINHTTP_OPTION_CONNECT_RETRIES, 1, context)?;
        io::check(context)?;
        // SAFETY: Two initialized DWORDs disable failed connection retry conditions before requests exist.
        unsafe {
            WinHttpSetOption(
                Some(raw(self.session)),
                WINHTTP_OPTION_FAILED_CONNECTION_RETRIES,
                Some(&[0; 8]),
            )
        }
        .map_err(|e| Failure::native("http_disable_failed_connection_retries", e))?;
        io::timeout(self.session, context, idle)?;
        let host = HSTRING::from(host);
        // SAFETY: Session and owned UTF-16 host remain live through this synchronous operation.
        self.connection =
            adopt(unsafe { WinHttpConnect(raw(self.session), &host, port, 0) }, "http_connect")?;
        io::check(context)
    }
    /// Create a closed GET policy on the retained connection without ambient authentication or redirects.
    fn request(
        &mut self,
        path: &str,
        secure: bool,
        context: &JobContext,
        idle: Option<u32>,
    ) -> Result<(), Failure> {
        io::check(context)?;
        if self.connection == 0
            || self.request != 0
            || path.len() > 8192
            || !path.starts_with('/')
            || path.chars().any(char::is_control)
        {
            return Err(Failure::invalid("http_request_input"));
        }
        let path = HSTRING::from(path);
        let mut flags = WINHTTP_FLAG_REFRESH | WINHTTP_FLAG_ESCAPE_DISABLE;
        if secure {
            flags |= WINHTTP_FLAG_SECURE;
        }
        // SAFETY: Retained connection, bounded owned path, static verb/version and no callbacks.
        self.request = adopt(
            unsafe {
                WinHttpOpenRequest(
                    raw(self.connection),
                    w!("GET"),
                    &path,
                    w!("HTTP/1.1"),
                    PCWSTR::null(),
                    std::ptr::null(),
                    flags,
                )
            },
            "http_request",
        )?;
        option(
            self.request,
            WINHTTP_OPTION_DISABLE_FEATURE,
            WINHTTP_DISABLE_AUTHENTICATION | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_REDIRECTS,
            context,
        )?;
        if idle.is_some() {
            option(
                self.request,
                WINHTTP_OPTION_AUTOLOGON_POLICY,
                WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH,
                context,
            )?;
        }
        option(
            self.request,
            WINHTTP_OPTION_REDIRECT_POLICY,
            WINHTTP_OPTION_REDIRECT_POLICY_NEVER,
            context,
        )?;
        option(self.request, WINHTTP_OPTION_MAX_RESPONSE_HEADER_SIZE, 16 * 1024, context)?;
        option(self.request, WINHTTP_OPTION_DECOMPRESSION, 0, context)?;
        io::timeout(self.request, context, idle)
    }
    /// Send the current readiness request without additional headers or body.
    /// # Errors
    /// Preserves exact native errors after checking the original cancellation/deadline.
    pub fn send_readiness(&mut self, context: &JobContext) -> Result<(), Failure> {
        if self.idle.is_some() {
            return Err(Failure::invalid("http_wrong_route"));
        }
        self.send(None, context)
    }
    /// Send an update GET with identity encoding and an optional resumed byte offset.
    /// # Errors
    /// Rejects invalid idle/custody and preserves exact native errors.
    pub fn send_update(
        &mut self,
        offset: Option<u64>,
        context: &JobContext,
    ) -> Result<(), Failure> {
        self.idle.ok_or_else(|| Failure::invalid("http_wrong_route"))?;
        let mut text = "Accept-Encoding: identity\r\n".to_owned();
        if let Some(offset) = offset {
            text.push_str(&format!("Range: bytes={offset}-\r\n"));
        }
        let headers: Vec<u16> = text.encode_utf16().collect();
        self.send(Some(&headers), context)
    }
    /// Send only the private route-selected headers, retaining their storage through the synchronous call.
    fn send(&mut self, headers: Option<&[u16]>, context: &JobContext) -> Result<(), Failure> {
        self.active(context)?;
        io::timeout(self.request, context, self.idle)?;
        // SAFETY: Synchronous retained GET, bounded optional headers and no body/callback context.
        let result = unsafe { WinHttpSendRequest(raw(self.request), headers, None, 0, 0, 0) };
        io::check(context)?;
        result.map_err(|e| Failure::native("http_send", e))
    }
    /// Receive only headers; redirect decisions remain with the feature owner.
    /// # Errors
    /// Rejects cancellation, elapsed deadlines and native failures without native retries.
    pub fn receive(&mut self, context: &JobContext) -> Result<(), Failure> {
        self.active(context)?;
        io::timeout(self.request, context, self.idle)?;
        // SAFETY: Retained synchronous request has no asynchronous reserved context.
        let result = unsafe { WinHttpReceiveResponse(raw(self.request), std::ptr::null_mut()) };
        io::check(context)?;
        result.map_err(|e| Failure::native("http_receive", e))
    }
    /// Close this response before another readiness GET; unsuccessful custody is retained.
    /// # Errors
    /// Returns the original close error without clearing the unsuccessful slot.
    pub fn close_request(&mut self) -> Result<(), Failure> {
        close_slot(&mut self.request)
    }
    /// Close children before parents, even after cancellation. Never releases failed slots.
    /// # Errors
    /// Stops at the first failed close so its parent remains retained for explicit retry.
    pub fn close(&mut self) -> Result<(), Failure> {
        self.close_using(|handle| {
            // SAFETY: Nonzero exclusive slot owns a live synchronous handle without concurrent users.
            unsafe { WinHttpCloseHandle(raw(handle)) }.map_err(|e| Failure::native("http_close", e))
        })
    }
    /// Clear only successfully retired children; a failed child prevents parent retirement.
    fn close_using(&mut self, close: impl Fn(usize) -> Result<(), Failure>) -> Result<(), Failure> {
        for handle in [&mut self.request, &mut self.connection, &mut self.session] {
            if *handle != 0 {
                close(*handle)?;
                *handle = 0;
            }
        }
        Ok(())
    }
    /// Reject native operations without an owned request slot.
    fn active(&self, context: &JobContext) -> Result<(), Failure> {
        self.validate_context(context)?;
        if self.request == 0 { Err(Failure::invalid("http_request_absent")) } else { Ok(()) }
    }
    /// Active native work cannot borrow a foreign admission or extend the original job cutoff.
    fn validate_context(&self, context: &JobContext) -> Result<(), Failure> {
        io::check(context)?;
        if self.context != Some(ContextIdentity::new(context)?) {
            return Err(Failure::invalid("http_context"));
        }
        Ok(())
    }
}
/// Borrow the private retained slot as a native pointer without exposing it to consumers.
fn raw(handle: usize) -> *mut std::ffi::c_void {
    handle as *mut _
}
/// Retain every successful native output immediately, preserving the original failed constructor code.
fn adopt(handle: *mut std::ffi::c_void, stage: &'static str) -> Result<usize, Failure> {
    if handle.is_null() {
        Err(Failure::native(stage, windows::core::Error::from_thread()))
    } else {
        Ok(handle as usize)
    }
}
/// Set one fixed DWORD under the original admitted cancellation/deadline checks.
fn option(handle: usize, key: u32, value: u32, context: &JobContext) -> Result<(), Failure> {
    io::check(context)?;
    // SAFETY: Retained handle and initialized DWORD bytes outlive the synchronous option call.
    unsafe { WinHttpSetOption(Some(raw(handle)), key, Some(&value.to_ne_bytes())) }
        .map_err(|e| Failure::native("http_option", e))?;
    io::check(context)
}
/// Retire exactly one owned slot and clear custody only after native success.
fn close_slot(slot: &mut usize) -> Result<(), Failure> {
    if *slot != 0 {
        // SAFETY: Exclusive retained slot owns the handle and clears it only after successful closure.
        unsafe { WinHttpCloseHandle(raw(*slot)) }.map_err(|e| Failure::native("http_close", e))?;
        *slot = 0;
    }
    Ok(())
}
#[cfg(test)]
mod tests;
