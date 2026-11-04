// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Locate the bulk channel at a reserved path on the connected frontend's own origin.
//!
//! The page's Content-Security-Policy may restrict `connect-src` to `'self'` (ComfyUI does so
//! under `--disable-api-nodes`), so the channel lives on the page's origin rather than a
//! separate one. The host intercepts every request under the reserved prefix in-process and
//! answers it itself; none reaches the network or the ComfyUI server.

use super::{Route, Token};
use crate::DesktopError;
use crate::bulk_channel_diagnostic::{Miss, Origin};

/// Reserved path segment appended to the frontend root the host navigates to.
///
/// ComfyUI's server answers `/`, its frontend's static assets, and the `/api`, `/internal`,
/// `/extensions`, `/templates` and `/docs` families plus the unprefixed legacy aliases of its
/// API routes; custom nodes register routes under their own names. None of them begins with
/// `__`, and this double-underscore, product-named segment belongs to SugarSubstitute alone.
/// Interception keeps the choice safe even against a server that did serve the path: every
/// request under it is answered in-process, so such a server never sees one.
pub(crate) const RESERVED_PATH: &str = "__sugarsubstitute/bulk/";

/// One intercepted request as the native adapter observed it, before any token is consumed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Request<'a> {
    /// Whether the browser classified it as a Fetch API request; other contexts never match.
    pub(crate) fetch: bool,
    /// Method, or `None` when it is not short ASCII text.
    pub(crate) method: Option<&'a str>,
    /// Absolute URL, or `None` when it is longer than [`Endpoint::MAX_ROUTE_BYTES`] or not ASCII.
    pub(crate) uri: Option<&'a str>,
    /// The `Origin` request header.
    pub(crate) origin: Header<'a>,
}

/// One optional request header; unreadable text never compares equal to anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Header<'a> {
    /// The request carries no such header.
    Absent,
    /// The header's exact short ASCII value.
    Text(&'a str),
    /// The header is present but too long, not ASCII or could not be read.
    Unreadable,
}

/// Two route bytes (`d/` or `u/`) plus the canonical 32-digit token.
const ROUTE_TAIL_BYTES: usize = 2 + 32;

/// The exact origin and reserved URL prefix the host intercepts for one connected frontend.
///
/// One main lifetime serves exactly one connection: its root never changes, and a different
/// connection or origin is a different main lifetime with its own endpoint and interception.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Endpoint {
    /// Serialized page origin (`scheme://host[:port]`), as browsers send it in `Origin`.
    origin: String,
    /// Absolute reserved path, including the root's base path (`/base/__sugarsubstitute/bulk/`).
    path: String,
    /// Every channel URL starts with exactly this text: `origin` followed by `path`.
    prefix: String,
}

impl Endpoint {
    /// Longest route URL the channel can match, in bytes; a longer root is refused.
    pub(crate) const MAX_ROUTE_BYTES: usize = 512;

    /// Derive the endpoint from the frontend root URL the host navigates to.
    ///
    /// `root` is an absolute `http` or `https` URL without credentials, query or fragment whose
    /// path names a directory (`http://127.0.0.1:8188/`, `https://comfy.example/base/`). The
    /// default port is elided and an internationalized host is serialized in ASCII, exactly as
    /// the browser serializes the page origin and request URLs.
    ///
    /// # Errors
    /// Refuses any other scheme, credentials, an opaque or missing host, a query or fragment,
    /// a path that is not a directory, and a root whose routes would exceed
    /// [`Self::MAX_ROUTE_BYTES`].
    pub(crate) fn for_root(root: &str) -> Result<Self, DesktopError> {
        let refuse = |reason| DesktopError::message("bulk_endpoint", reason);
        let url =
            url::Url::parse(root).map_err(|cause| DesktopError::new("bulk_endpoint", cause))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(refuse("frontend root is not HTTP"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(refuse("frontend root carries credentials"));
        }
        if url.host().is_none() || url.query().is_some() || url.fragment().is_some() {
            return Err(refuse("frontend root is not a plain origin and path"));
        }
        if !url.path().ends_with('/') {
            return Err(refuse("frontend root path is not a directory"));
        }
        let origin = url.origin().ascii_serialization();
        let path = format!("{}{RESERVED_PATH}", url.path());
        let prefix = format!("{origin}{path}");
        if prefix.len() + ROUTE_TAIL_BYTES > Self::MAX_ROUTE_BYTES || !prefix.is_ascii() {
            return Err(refuse("frontend root exceeds the bulk route bound"));
        }
        Ok(Self { origin, path, prefix })
    }

    /// Borrow the absolute reserved path the bridge appends to `location.origin`.
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    /// Borrow the URL prefix every intercepted request starts with.
    #[cfg(test)]
    pub(crate) fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Return the WebView2 request filter covering the reserved prefix and nothing else.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn filter(&self) -> String {
        format!("{}*", self.prefix)
    }

    /// Match one intercepted request to a live operation without consuming anything.
    ///
    /// The request must be a Fetch API request with the exact method and URL: there is no query,
    /// fragment, alternate spelling or trailing segment. A present `Origin` header must be this
    /// page's origin. An absent one is admitted because browsers omit it from same-origin `GET`
    /// requests; the single-use token remains the capability either way.
    ///
    /// # Errors
    /// Returns the first failed check, in the order URL, prefix, route, capability form, origin
    /// and resource context, so a later [`Miss`] proves every earlier check passed. The caller
    /// answers any miss with a local refusal.
    pub(crate) fn route(&self, request: Request<'_>) -> Result<Route, Miss> {
        let path = request
            .uri
            .ok_or(Miss::UrlUnreadable)?
            .strip_prefix(self.prefix.as_str())
            .ok_or(Miss::PrefixMismatch)?;
        let (route, token) = path.split_at_checked(2).ok_or(Miss::RouteMismatch)?;
        let route: fn(Token) -> Route = match (request.method, route) {
            (Some("GET"), "d/") => Route::Download,
            (Some("POST"), "u/") => Route::Upload,
            _ => return Err(Miss::RouteMismatch),
        };
        let token = Token::parse(token).ok_or(Miss::MalformedCapability)?;
        if !matches!(self.origin(request.origin), Origin::Absent | Origin::Matching) {
            return Err(Miss::ForeignOrigin);
        }
        if !request.fetch {
            return Err(Miss::NonFetchContext);
        }
        Ok(route(token))
    }

    /// Classify a request's `Origin` header against this page's origin.
    pub(crate) fn origin(&self, header: Header<'_>) -> Origin {
        match header {
            Header::Absent => Origin::Absent,
            Header::Text(origin) if origin == self.origin => Origin::Matching,
            Header::Text(_) => Origin::Other,
            Header::Unreadable => Origin::Unreadable,
        }
    }
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
