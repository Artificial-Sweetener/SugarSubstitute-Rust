// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound browser addresses and admit only the exact configured HTTP loopback origin.

use sugarsubstitute_contracts::configuration::{LoopbackHost, LoopbackOrigin};

/// Maximum untrusted native navigation URL length, in UTF-8 bytes.
pub(crate) const MAX_URL_BYTES: usize = 4096;

/// Construct the sole initial served root URL from validated domain data.
pub(crate) fn root_url(origin: LoopbackOrigin) -> String {
    let host = match origin.host() {
        LoopbackHost::Ipv4 => "127.0.0.1",
        LoopbackHost::Ipv6 => "[::1]",
        LoopbackHost::Localhost => "localhost",
    };
    format!("http://{host}:{}/", origin.port())
}

/// Validate raw authority before URL parsing can normalize alternate host spelling.
/// Frame and top-document paths remain with the unchanged served frontend.
pub(crate) fn admits_url(origin: LoopbackOrigin, value: &str) -> bool {
    if value.len() > MAX_URL_BYTES || value.contains('\0') {
        return false;
    }
    let Some(rest) = value.strip_prefix("http://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let Ok(observed) = LoopbackOrigin::parse(&value[..7 + authority.len()], origin.port().get())
    else {
        return false;
    };
    if observed != origin {
        return false;
    }
    url::Url::parse(value).is_ok_and(|parsed| {
        parsed.scheme() == "http" && parsed.username().is_empty() && parsed.password().is_none()
    })
}
