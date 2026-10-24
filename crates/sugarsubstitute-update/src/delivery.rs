// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interpret HTTP range responses against authenticated package bounds before writing bytes.
use crate::Target;

/// Selected response fields after the transport rejects duplicate/ambiguous framing headers.
/// The transport must request identity encoding and expose the decoded HTTP message body,
/// without content decompression. Redirect and origin policy remain with that transport.
pub struct Response<'a> {
    /// Final HTTP status; redirects are not package data.
    pub status: u16,
    /// Optional unique Content-Length, parsed as an unsigned decimal by the transport.
    pub length: Option<u64>,
    /// Unique Content-Range, when present.
    pub range: Option<&'a str>,
    /// Unique Content-Encoding, when present.
    pub encoding: Option<&'a str>,
}

/// A response cannot supply the requested immutable package interval.
#[derive(Debug, Eq, PartialEq)]
pub enum Rejection {
    /// Unexpected HTTP status; retain it for retry/rate-limit policy.
    Status(u16),
    /// The response changes content encoding or supplies inconsistent range/length fields.
    Framing,
    /// The supplied resume offset exceeds the authenticated target length.
    Offset,
    /// Actual body length differs from the authenticated interval.
    BodyLength,
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Update response rejected: {self:?}")
    }
}
impl std::error::Error for Rejection {}

/// Exact write interval; a 200 response restarts an owned partial instead of appending it.
pub struct Transfer {
    offset: u64,
    length: u64,
}
impl Transfer {
    /// Validate response framing using the signed target's total, never a server-supplied total.
    /// # Errors
    /// Rejects nonidentity content, unexpected statuses and changed/overlapping ranges.
    /// A shorter contiguous interval is allowed; the next request resumes after it.
    pub fn accept(target: &Target, offset: u64, response: Response<'_>) -> Result<Self, Rejection> {
        if offset > target.length() {
            return Err(Rejection::Offset);
        }
        if response.encoding.is_some_and(|value| !value.eq_ignore_ascii_case("identity")) {
            return Err(Rejection::Framing);
        }
        let (start, length) = match response.status {
            200 if response.range.is_none() => (0, target.length()),
            200 => return Err(Rejection::Framing),
            206 => {
                let (first, last, total) = range(response.range.ok_or(Rejection::Framing)?)?;
                if first != offset || total != target.length() {
                    return Err(Rejection::Framing);
                }
                (offset, last - first + 1)
            }
            status => return Err(Rejection::Status(status)),
        };
        if response.length.is_some_and(|declared| declared != length) {
            return Err(Rejection::Framing);
        }
        Ok(Self { offset: start, length })
    }
    /// Offset at which the response body belongs; zero authorizes restarting the owned partial.
    pub fn offset(&self) -> u64 {
        self.offset
    }
    /// Exact body bytes required for this response.
    pub fn length(&self) -> u64 {
        self.length
    }
}

/// Parse the single requested byte range without accepting multipart or wildcard totals.
fn range(value: &str) -> Result<(u64, u64, u64), Rejection> {
    let (unit, value) = value.split_once(' ').ok_or(Rejection::Framing)?;
    if !unit.eq_ignore_ascii_case("bytes") {
        return Err(Rejection::Framing);
    }
    let (interval, total) = value.split_once('/').ok_or(Rejection::Framing)?;
    let (first, last) = interval.split_once('-').ok_or(Rejection::Framing)?;
    let (first, last, total) = (decimal(first)?, decimal(last)?, decimal(total)?);
    if first > last || last >= total {
        return Err(Rejection::Framing);
    }
    Ok((first, last, total))
}
/// Bound numeric fields before parsing; signs, whitespace and overflow never alias a range.
fn decimal(value: &str) -> Result<u64, Rejection> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Rejection::Framing);
    }
    value.parse().map_err(|_| Rejection::Framing)
}

#[cfg(test)]
mod tests;
