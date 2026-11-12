// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Admit only exact loopback HTTP root origins without credentials or URL payloads.

use super::ConfigurationError;
use std::num::NonZeroU16;

/// The three explicitly supported loopback host spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopbackHost {
    /// IPv4 loopback, spelled exactly `127.0.0.1`.
    Ipv4,
    /// IPv6 loopback, spelled exactly `[::1]` in a URL.
    Ipv6,
    /// The exact local hostname `localhost`.
    Localhost,
}

/// A root HTTP origin with an admitted host and matching explicit backend port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopbackOrigin {
    host: LoopbackHost,
    port: NonZeroU16,
}

impl LoopbackOrigin {
    /// Admit an exact HTTP root URL and require its effective port to match `port`.
    ///
    /// Only an empty path or `/` is accepted. The optional URL port defaults to
    /// HTTP port 80; the configuration port is always explicit and nonzero.
    /// Credentials, queries, fragments, alternate host spellings and normalization
    /// are rejected. Returns a typed origin, port or mismatch error.
    pub fn parse(url: &str, port: u16) -> Result<Self, ConfigurationError> {
        let port = NonZeroU16::new(port).ok_or(ConfigurationError::InvalidPort)?;
        let authority =
            url.strip_prefix("http://").ok_or(ConfigurationError::InvalidAttachOrigin)?;
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        let (host, suffix) = if let Some(suffix) = authority.strip_prefix("[::1]") {
            (LoopbackHost::Ipv6, suffix)
        } else if let Some(suffix) = authority.strip_prefix("127.0.0.1") {
            (LoopbackHost::Ipv4, suffix)
        } else if let Some(suffix) = authority.strip_prefix("localhost") {
            (LoopbackHost::Localhost, suffix)
        } else {
            return Err(ConfigurationError::InvalidAttachOrigin);
        };
        let url_port = if suffix.is_empty() {
            80
        } else {
            let value = suffix.strip_prefix(':').ok_or(ConfigurationError::InvalidAttachOrigin)?;
            if value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || (value.len() > 1 && value.starts_with('0'))
            {
                return Err(ConfigurationError::InvalidAttachOrigin);
            }
            value.parse::<u16>().map_err(|_| ConfigurationError::InvalidAttachOrigin)?
        };
        if url_port == 0 {
            return Err(ConfigurationError::InvalidPort);
        }
        if url_port != port.get() {
            return Err(ConfigurationError::AttachPortMismatch);
        }
        Ok(Self { host, port })
    }
    /// Return the admitted host identity without an arbitrary URL string.
    pub fn host(self) -> LoopbackHost {
        self.host
    }
    /// Return the matching explicit backend port.
    pub fn port(self) -> NonZeroU16 {
        self.port
    }
}
