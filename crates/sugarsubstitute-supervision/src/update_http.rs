// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own finite synchronous update HTTP calls and observe native retirement before delivery.
mod failure;
#[allow(unsafe_code)]
mod native;
use crate::setup_storage::Download;
pub use failure::Failure;
use native::{Body, Handles};
use std::{
    io::{self, Read},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
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
    handles: Handles,
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
        self.handles.close()?;
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
        native::milliseconds(context, idle)?;
        self.memory = Some(context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?);
        let outcome = (|| {
            for redirects in 0..=5 {
                policy.admits(&endpoint)?;
                let headers = native::open(&mut self.handles, &endpoint, offset, idle, context)?;
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
                    self.handles.close()?;
                    continue;
                }
                return consume(&headers, &mut Body::new(&self.handles, idle, context));
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
