// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own finite synchronous update HTTP calls and observe native retirement before delivery.
mod failure;
use crate::setup_storage::Download;
pub use failure::Failure;
use std::{
    io::{self, Read, Write},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_networking::Transport;
use sugarsubstitute_update::{
    MAX_METADATA_BYTES,
    http::{Endpoint, Headers, Policy},
};

/// Additional admission beyond download storage; includes metadata and header buffers.
pub const RESERVATION_BYTES: u64 = 4 * 1024 * 1024;

/// Retained native custody for a request owner, created before its execution job is submitted.
/// Keep this owner outside discardable job results until `settled()` is true. All native calls
/// run on admitted workers; exclusive borrowing prevents concurrent access to synchronous handles.
#[derive(Default)]
pub struct Http {
    handles: Transport,
    memory: Option<MemoryClaim>,
}
impl Http {
    /// Construct inert custody without opening sockets, native handles or tasks.
    pub fn new() -> Self {
        Self::default()
    }
    /// Whether every native handle and its associated memory claim has actually retired.
    pub fn settled(&self) -> bool {
        self.handles.empty() && self.memory.is_none()
    }
    /// Retry original native closures on an admitted cleanup worker, even after cancellation.
    /// # Errors
    /// Retains each unresolved handle and its claim for another explicit retirement attempt.
    pub fn retire(&mut self) -> io::Result<()> {
        self.handles.close().map_err(io::Error::from)?;
        self.memory = None;
        Ok(())
    }
    /// Fetch bounded metadata bytes without granting them authenticity.
    /// The caller authenticates the returned bytes and commits trust before using an offer.
    /// # Errors
    /// Rejects destination/framing violations, excess/truncated bodies, cancellation and I/O.
    pub fn metadata(
        &mut self,
        policy: &Policy,
        endpoint: Endpoint,
        maximum: usize,
        idle: Duration,
        context: &JobContext,
    ) -> Result<Vec<u8>, Failure> {
        if maximum == 0 || maximum > MAX_METADATA_BYTES {
            return Err(invalid("Update metadata bound invalid").into());
        }
        self.request(policy, endpoint, None, idle, context, |headers, body| {
            headers.metadata(maximum)?;
            let mut bytes = Vec::new();
            body.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > maximum || headers.length().is_some_and(|n| n != bytes.len() as u64) {
                return Err(invalid("Update metadata body length invalid").into());
            }
            Ok(bytes)
        })
    }
    /// Stream one complete, unauthenticated response body of at most `maximum` bytes into
    /// `output`, for a caller that verifies the bytes afterwards (a node-pack archive).
    /// Returns the bytes written.
    /// # Errors
    /// Rejects destination/framing violations, a declared or actual body over `maximum`, a
    /// truncated body, cancellation and I/O; `output` may then hold a prefix.
    pub fn file(
        &mut self,
        policy: &Policy,
        endpoint: Endpoint,
        maximum: u64,
        idle: Duration,
        context: &JobContext,
        output: &mut dyn Write,
    ) -> Result<u64, Failure> {
        let bound = usize::try_from(maximum).map_err(|_| invalid("Download bound invalid"))?;
        if maximum == 0 {
            return Err(invalid("Download bound invalid").into());
        }
        self.request(policy, endpoint, None, idle, context, |headers, body| {
            headers.metadata(bound)?;
            let written = io::copy(&mut body.take(maximum + 1), output)?;
            if written > maximum || headers.length().is_some_and(|n| n != written) {
                return Err(invalid("Download body length invalid").into());
            }
            Ok(written)
        })
    }
    /// Transfer one response into a retained resumable entry. A shorter valid range advances
    /// its offset; the coordinator decides whether to request another interval in this job.
    /// # Errors
    /// Preserves the cache's received prefix on interruption and observes HTTP cleanup on error.
    pub fn package(
        &mut self,
        policy: &Policy,
        endpoint: Endpoint,
        download: &mut Download,
        idle: Duration,
        context: &JobContext,
    ) -> Result<(), Failure> {
        let offset = download.offset()?;
        self.request(policy, endpoint, Some(offset), idle, context, |headers, body| {
            download.receive(headers.package_response(), body, context)?;
            Ok(())
        })
    }
    /// A request either retires all handles or leaves this same owner available for cleanup.
    fn request<T>(
        &mut self,
        policy: &Policy,
        mut endpoint: Endpoint,
        offset: Option<u64>,
        idle: Duration,
        context: &JobContext,
        consume: impl FnOnce(&Headers, &mut Body<'_>) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if !self.settled() {
            return Err(invalid("Previous update HTTP custody is unresolved").into());
        }
        check(context)?;
        idle_milliseconds(context, idle)?;
        self.memory = Some(context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?);
        let outcome = (|| {
            for redirects in 0..=5 {
                policy.admits(&endpoint)?;
                let idle_ms = idle_milliseconds(context, idle)?;
                self.handles
                    .update(
                        &transport_request(&endpoint, &endpoint.request_target())?,
                        idle_ms,
                        context,
                    )
                    .map_err(io::Error::from)?;
                self.handles.send_update(offset, context).map_err(io::Error::from)?;
                self.handles.receive(context).map_err(io::Error::from)?;
                let response = self.handles.response(context).map_err(io::Error::from)?;
                let headers = Headers::parse(response.status, &response.headers)?;
                if matches!(headers.status(), 301 | 302 | 303 | 307 | 308) {
                    if redirects == 5 {
                        return Err(invalid("Update redirect limit reached").into());
                    }
                    endpoint = policy.redirect(
                        &endpoint,
                        headers
                            .location()
                            .ok_or_else(|| invalid("Update redirect has no location"))?,
                    )?;
                    self.handles.close().map_err(io::Error::from)?;
                    continue;
                }
                return consume(&headers, &mut Body { handles: &mut self.handles, context });
            }
            Err(invalid("Update redirect state invalid").into())
        })();
        match (outcome, self.retire()) {
            (Ok(value), Ok(())) => {
                check(context)?;
                Ok(value)
            }
            (Err(cause), Ok(())) => Err(cause),
            (Ok(_), Err(cause)) => Err(cause.into()),
            (Err(operation), Err(retirement)) => {
                Err(Failure::Retirement { operation: Box::new(operation), retirement })
            }
        }
    }
}
impl Drop for Http {
    fn drop(&mut self) {
        if !self.handles.empty() {
            tracing::error!(
                operation = "update_http_unretired",
                "Update HTTP owner dropped with unresolved native custody"
            );
            // Never claim that unresolved native resources released their admission charge.
            if let Some(memory) = self.memory.take() {
                std::mem::forget(memory);
            }
        }
    }
}
/// Stop between bounded native calls; the original job deadline is never extended.
fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
/// Produce bounded diagnostics without logging URLs, queries, headers or credentials.
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;

/// Bound idle policy separately from the original job deadline before memory admission.
fn idle_milliseconds(context: &JobContext, idle: Duration) -> io::Result<u32> {
    check(context)?;
    if idle < Duration::from_millis(1) || idle > Duration::from_secs(120) {
        return Err(invalid("HTTP idle timeout outside supported bounds"));
    }
    sugarsubstitute_networking::milliseconds(
        context.deadline(),
        context.now(),
        Some(idle.as_millis() as u32),
    )
    .map_err(|cause| io::Error::new(io::ErrorKind::TimedOut, cause))?;
    Ok(idle.as_millis() as u32)
}
/// The body borrow cannot outlive retained native custody or the original admitted context.
struct Body<'a> {
    handles: &'a mut Transport,
    context: &'a JobContext,
}
impl Read for Body<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.handles.read(bytes, self.context).map_err(io::Error::from)
    }
}

/// Adapt only this feature's already origin-authorized endpoint to closed transport controls.
fn transport_request<'a>(
    endpoint: &'a Endpoint,
    target: &'a str,
) -> io::Result<sugarsubstitute_networking::UpdateRequest<'a>> {
    if endpoint.secure() {
        sugarsubstitute_networking::UpdateRequest::https(
            endpoint.host().map_err(io::Error::other)?,
            endpoint.port().map_err(io::Error::other)?,
            target,
        )
        .map_err(io::Error::other)
    } else {
        #[cfg(any(test, feature = "boundary-fixtures"))]
        {
            if endpoint.host().map_err(io::Error::other)? != "127.0.0.1" {
                return Err(invalid("Update HTTP fixture must be literal loopback"));
            }
            sugarsubstitute_networking::UpdateRequest::loopback(
                endpoint.port().map_err(io::Error::other)?,
                target,
            )
            .map_err(io::Error::other)
        }
        #[cfg(not(any(test, feature = "boundary-fixtures")))]
        {
            Err(invalid("Update transport requires HTTPS"))
        }
    }
}
