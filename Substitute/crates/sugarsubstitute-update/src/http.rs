// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound transport destinations and HTTP framing independently of metadata authenticity.
mod headers;
pub use headers::Headers;
use url::Url;

/// Transport input violated destination or framing policy.
#[derive(Debug, Eq, PartialEq)]
pub enum Rejection {
    /// Malformed, credential-bearing or oversized URL/target name.
    Address,
    /// Destination is outside the locally configured origin allowlist.
    Origin,
    /// Response headers are ambiguous, malformed, encoded or oversized.
    Headers,
    /// Non-success metadata response, retained for discovery and retry decisions.
    Status(u16),
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Update HTTP input rejected: {self:?}")
    }
}
impl std::error::Error for Rejection {}

/// A locally configured feed base and exact permitted redirect origins, including ports.
/// Server responses cannot extend this allowlist or select a different repository policy.
pub struct Policy {
    base: Url,
    origins: Vec<String>,
}
/// Validated transport address. Its query may contain temporary asset credentials; never log it.
pub struct Endpoint(Url);
impl Policy {
    /// Configure an HTTPS feed and optional HTTPS redirect origins, such as a release CDN.
    /// # Errors
    /// Rejects credentials, fragments, queries on bases, non-HTTPS addresses and excessive lists.
    pub fn https(base: &str, redirects: &[&str]) -> Result<Self, Rejection> {
        let base = address(base)?;
        if base.scheme() != "https"
            || base.query().is_some()
            || !base.path().ends_with('/')
            || redirects.len() > 8
        {
            return Err(Rejection::Address);
        }
        let mut origins = vec![base.origin().ascii_serialization()];
        for redirect in redirects {
            let value = address(redirect)?;
            if value.scheme() != "https" || value.path() != "/" || value.query().is_some() {
                return Err(Rejection::Address);
            }
            let origin = value.origin().ascii_serialization();
            if !origins.contains(&origin) {
                origins.push(origin);
            }
        }
        Ok(Self { base, origins })
    }
    /// Bind an isolated loopback fixture; HTTP support is absent from normal production builds.
    /// # Errors
    /// Rejects port zero. Redirects remain restricted to this exact literal origin and port.
    #[cfg(any(test, feature = "boundary-fixtures"))]
    pub fn loopback(port: u16) -> Result<Self, Rejection> {
        if port == 0 {
            return Err(Rejection::Address);
        }
        let base = address(&format!("http://127.0.0.1:{port}/"))?;
        Ok(Self { origins: vec![base.origin().ascii_serialization()], base })
    }
    /// Resolve one flat release asset name under the configured base.
    /// # Errors
    /// Rejects path traversal, separators, query injection and unbounded names.
    pub fn asset(&self, name: &str) -> Result<Endpoint, Rejection> {
        if name.is_empty()
            || name.len() > 1024
            || matches!(name, "." | "..")
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(Rejection::Address);
        }
        Ok(Endpoint(self.base.join(name).map_err(|_| Rejection::Address)?))
    }
    /// Resolve a redirect without inheriting native automatic-authentication or redirect policy.
    /// # Errors
    /// Rejects foreign origins, credentials, fragments and HTTPS downgrade.
    pub fn redirect(&self, current: &Endpoint, location: &str) -> Result<Endpoint, Rejection> {
        bounded(location)?;
        let next = current.0.join(location).map_err(|_| Rejection::Address)?;
        let next = address(next.as_str())?;
        if !self.origins.contains(&next.origin().ascii_serialization()) {
            return Err(Rejection::Origin);
        }
        Ok(Endpoint(next))
    }
    /// Check that an endpoint belongs to this policy, including endpoints from other callers.
    /// # Errors
    /// Rejects cross-policy endpoints before any connection is opened.
    pub fn admits(&self, endpoint: &Endpoint) -> Result<(), Rejection> {
        if self.origins.contains(&endpoint.0.origin().ascii_serialization()) {
            Ok(())
        } else {
            Err(Rejection::Origin)
        }
    }
}
impl Endpoint {
    /// Validated server name without user credentials.
    pub fn host(&self) -> Result<&str, Rejection> {
        self.0.host_str().ok_or(Rejection::Address)
    }
    /// Explicit or scheme-default port.
    pub fn port(&self) -> Result<u16, Rejection> {
        self.0.port_or_known_default().ok_or(Rejection::Address)
    }
    /// Whether native certificate-validated TLS is required.
    pub fn secure(&self) -> bool {
        self.0.scheme() == "https"
    }
    /// Exact escaped request target, including a possible temporary CDN query.
    pub fn request_target(&self) -> String {
        match self.0.query() {
            Some(query) => format!("{}?{query}", self.0.path()),
            None => self.0.path().to_owned(),
        }
    }
}
/// Validate raw URL shape before URL normalization can hide whitespace or credentials.
fn bounded(value: &str) -> Result<(), Rejection> {
    if value.is_empty()
        || value.len() > 8192
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(Rejection::Address);
    }
    Ok(())
}
/// Parse only hierarchical HTTP(S) addresses with no ambient credentials or fragment.
fn address(value: &str) -> Result<Url, Rejection> {
    bounded(value)?;
    let parsed = Url::parse(value).map_err(|_| Rejection::Address)?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(Rejection::Address);
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests;
