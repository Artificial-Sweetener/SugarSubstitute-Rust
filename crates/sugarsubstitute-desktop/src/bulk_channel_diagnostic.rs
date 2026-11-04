// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed schema-one facts about bulk channel requests the host refused.
//!
//! The record names why a request under the reserved bulk path was answered with a local 404,
//! together with the browser's raw resource context, a closed method class and how its `Origin`
//! header relates to the page. It never carries the request URL, token or body, and it grants
//! no capability. The owned child writes it to its authenticated stderr, where the parent's
//! bounded decoder relays it as its own structured event.

use std::fmt;

/// The first check an intercepted bulk channel request failed.
///
/// Route checks run in this order: URL, prefix, route, capability form, origin, then resource
/// context. A later reason therefore proves every earlier check passed. Token outcomes follow
/// a fully matched route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Miss {
    /// The URL is longer than the route bound or not ASCII.
    UrlUnreadable,
    /// The URL does not start with this page's reserved bulk prefix.
    PrefixMismatch,
    /// The method and path under the prefix name no route (`GET d/` or `POST u/`).
    RouteMismatch,
    /// The route's capability is not the canonical 32-digit lowercase hexadecimal form.
    MalformedCapability,
    /// A present `Origin` header is another origin or could not be read.
    ForeignOrigin,
    /// The browser did not classify the request as a Fetch API request.
    NonFetchContext,
    /// No staged download has this capability; it was never issued or already served.
    DownloadUnknown,
    /// The staged download outlived its single-use lifetime.
    DownloadExpired,
    /// The staged download belongs to a panel document that is no longer current.
    DownloadStale,
    /// No live upload grant has this capability.
    UploadUnknown,
    /// The upload grant belongs to a panel document that is no longer current.
    UploadStale,
    /// The upload's panel document closed while its body was read.
    UploadClosed,
    /// The upload body is empty, too large or not UTF-8.
    UploadSize,
    /// Upload storage could not be claimed from the session partition.
    UploadMemory,
    /// The native request body could not be read.
    UploadRead,
    /// Staged message retention was exhausted while the upload was accepted.
    UploadCapacity,
    /// A native step failed while the upload was accepted.
    UploadNative,
}

impl Miss {
    const ALL: [Self; 17] = [
        Self::UrlUnreadable,
        Self::PrefixMismatch,
        Self::RouteMismatch,
        Self::MalformedCapability,
        Self::ForeignOrigin,
        Self::NonFetchContext,
        Self::DownloadUnknown,
        Self::DownloadExpired,
        Self::DownloadStale,
        Self::UploadUnknown,
        Self::UploadStale,
        Self::UploadClosed,
        Self::UploadSize,
        Self::UploadMemory,
        Self::UploadRead,
        Self::UploadCapacity,
        Self::UploadNative,
    ];

    /// The stable wire name; none contains text the diagnostic formatter redacts.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UrlUnreadable => "url_unreadable",
            Self::PrefixMismatch => "prefix_mismatch",
            Self::RouteMismatch => "route_mismatch",
            Self::MalformedCapability => "malformed_capability",
            Self::ForeignOrigin => "foreign_origin",
            Self::NonFetchContext => "non_fetch_context",
            Self::DownloadUnknown => "download_unknown",
            Self::DownloadExpired => "download_expired",
            Self::DownloadStale => "download_stale",
            Self::UploadUnknown => "upload_unknown",
            Self::UploadStale => "upload_stale",
            Self::UploadClosed => "upload_closed",
            Self::UploadSize => "upload_size",
            Self::UploadMemory => "upload_memory",
            Self::UploadRead => "upload_read",
            Self::UploadCapacity => "upload_capacity",
            Self::UploadNative => "upload_native",
        }
    }
}

/// The request method, reduced to the closed set the channel distinguishes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    /// `GET`, the download method.
    Get,
    /// `POST`, the upload method.
    Post,
    /// `HEAD`.
    Head,
    /// `OPTIONS`, as a CORS preflight sends.
    Options,
    /// Any other readable method; its text is not recorded.
    Other,
    /// The method is not short ASCII text.
    Unreadable,
}

impl Method {
    const ALL: [Self; 6] =
        [Self::Get, Self::Post, Self::Head, Self::Options, Self::Other, Self::Unreadable];

    /// Classify the exact method text the browser reported, or `None` when it was unreadable.
    pub fn classify(method: Option<&str>) -> Self {
        match method {
            Some("GET") => Self::Get,
            Some("POST") => Self::Post,
            Some("HEAD") => Self::Head,
            Some("OPTIONS") => Self::Options,
            Some(_) => Self::Other,
            None => Self::Unreadable,
        }
    }

    /// The stable wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Other => "other",
            Self::Unreadable => "unreadable",
        }
    }
}

/// How a request's `Origin` header relates to the page's own origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin {
    /// The request carries no `Origin` header, as same-origin `GET` requests do.
    Absent,
    /// The header is exactly the page's origin.
    Matching,
    /// The header names another origin; its text is not recorded.
    Other,
    /// The header is present but too long, not ASCII or could not be read.
    Unreadable,
}

impl Origin {
    const ALL: [Self; 4] = [Self::Absent, Self::Matching, Self::Other, Self::Unreadable];

    /// The stable wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Matching => "matching",
            Self::Other => "other",
            Self::Unreadable => "unreadable",
        }
    }
}

impl fmt::Display for Miss {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for Method {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One refused bulk channel request, as numeric and closed-vocabulary facts only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    /// The first check the request failed.
    pub miss: Miss,
    /// The raw `COREWEBVIEW2_WEB_RESOURCE_CONTEXT` value the browser reported.
    pub context: i32,
    /// The request method's class.
    pub method: Method,
    /// How the request's `Origin` header relates to the page.
    pub origin: Origin,
}

/// The exact producer target, which the parser requires before the closed fields.
const TARGET: &str = "sugarsubstitute_desktop::bulk_channel_diagnostic: ";
/// The closed operation code shared by producer and parser.
const MARKER: &str = "operation=\"bulk_channel_refused\" ";
/// The relay's line bound; longer lines are never decoded.
const LINE_BYTES: usize = 512;

impl Record {
    /// Emit the schema-one record at `info`, in the field order [`Record::parse`] requires.
    pub fn emit(self) {
        tracing::info!(
            operation = "bulk_channel_refused",
            schema = 1,
            reason = %self.miss,
            context = i64::from(self.context),
            method = %self.method,
            origin = %self.origin
        );
    }

    /// Decode one complete schema-one line from this producer under the 512-byte line bound.
    /// Unknown, duplicate, extra or noncanonical fields are rejected.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > LINE_BYTES {
            return None;
        }
        let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = line.split_once(MARKER)?;
        if prefix.contains("operation=") || !prefix.ends_with(TARGET) {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "1" {
            return None;
        }
        let reason = field("reason")?;
        let miss = Miss::ALL.into_iter().find(|miss| miss.as_str() == reason)?;
        let context = context(field("context")?)?;
        let method = field("method")?;
        let method = Method::ALL.into_iter().find(|known| known.as_str() == method)?;
        let origin = field("origin")?;
        let origin = Origin::ALL.into_iter().find(|known| known.as_str() == origin)?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self { miss, context, method, origin })
    }
}

/// Accept a canonical decimal `i32`, rejecting padding, a plus sign and negative zero.
fn context(raw: &str) -> Option<i32> {
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || (digits == "0" && raw.starts_with('-'))
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    raw.parse().ok()
}

#[cfg(test)]
#[path = "bulk_channel_diagnostic_tests.rs"]
mod tests;
