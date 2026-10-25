// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve Windows proxy policy per destination without moving network work onto the UI thread.
#[allow(unsafe_code)]
mod native;
#[cfg(test)]
mod tests;
use std::io;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// Admission retained until the resolver's final native callback has been observed.
pub const RESERVATION_BYTES: u64 = 256 * 1024;

/// One ordered Windows routing decision. Direct access is allowed only when returned by Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Connect to the destination without a proxy.
    Direct,
    /// Connect through this HTTP or HTTPS proxy URL, without embedded credentials.
    Proxy {
        /// Canonical HTTP(S) origin with an explicit proxy port when non-default.
        url: String,
    },
}

/// Native custody retained outside discardable execution results until `settled()`.
/// Resolve and retire on admitted blocking workers, with exclusive access to this owner.
#[derive(Default)]
pub struct Resolver {
    native: native::Resolver,
    memory: Option<MemoryClaim>,
}
impl Resolver {
    /// Construct inert custody; this does not discover settings or start work.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether all native handles, callbacks and admission have retired.
    pub fn settled(&self) -> bool {
        self.native.settled() && self.memory.is_none()
    }

    /// Resolve the current user's static, PAC and network-specific proxy settings for a URL.
    /// # Errors
    /// Rejects invalid URLs, unsupported proxy schemes, OS errors, cancellation and expiry.
    /// A failed retirement retains custody here for an explicit `retire()` attempt.
    pub fn resolve(&mut self, address: &str, context: &JobContext) -> io::Result<Vec<Route>> {
        self.resolve_with(address, None, context)
    }

    /// Share production resolution with isolated PAC fixtures while retaining identical custody.
    fn resolve_with(
        &mut self,
        address: &str,
        pac: Option<&str>,
        context: &JobContext,
    ) -> io::Result<Vec<Route>> {
        if !self.settled() {
            return Err(io::Error::other("Previous proxy resolution has not retired"));
        }
        check(context)?;
        let url = url::Url::parse(address).map_err(io::Error::other)?;
        if address.len() > 8192
            || !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid proxy resolution destination",
            ));
        }
        self.memory = Some(context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?);
        let result = self.native.resolve(url.as_str(), pac, context);
        match (result, self.retire()) {
            (result, Ok(())) => result,
            (Ok(_), Err(cause)) => Err(cause),
            (Err(operation), Err(retirement)) => Err(io::Error::other(format!(
                "Proxy resolution failed: {operation}; retirement failed: {retirement}"
            ))),
        }
    }

    /// Close original handles and observe final callbacks, including after job cancellation.
    /// # Errors
    /// Retains unresolved custody and admission when native retirement has not finished.
    pub fn retire(&mut self) -> io::Result<()> {
        self.native.retire()?;
        self.memory = None;
        Ok(())
    }
}
impl Drop for Resolver {
    fn drop(&mut self) {
        if !self.settled() {
            tracing::error!(
                operation = "system_proxy_unretired",
                "Proxy resolver dropped with unresolved custody"
            );
            if let Some(memory) = self.memory.take() {
                std::mem::forget(memory);
            }
        }
    }
}
/// Check the original admitted deadline; discovery never extends it.
fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
