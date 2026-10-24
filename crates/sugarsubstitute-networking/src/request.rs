// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validated update request shape without feature origin, retry or authenticity authority.
use url::Url;
/// One bounded HTTPS update GET; automatic proxy selection and certificate validation are fixed.
/// Feature owners must independently authorize the destination and authenticate returned data.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct UpdateRequest<'a> {
    pub(crate) host: &'a str,
    pub(crate) port: u16,
    pub(crate) target: &'a str,
    pub(crate) secure: bool,
}
/// A request's supplied components do not describe one bounded origin-form GET.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidRequest;
impl std::fmt::Display for InvalidRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Invalid HTTP request components")
    }
}
impl std::error::Error for InvalidRequest {}
impl<'a> UpdateRequest<'a> {
    /// Bind already feature-authorized HTTPS components without granting response authenticity.
    /// # Errors
    /// Rejects credentials, authority/path injection, controls, fragments and excessive bounds.
    pub fn https(host: &'a str, port: u16, target: &'a str) -> Result<Self, InvalidRequest> {
        validate(host, port, target)?;
        Ok(Self { host, port, target, secure: true })
    }
    /// Bind an isolated literal-loopback HTTP fixture with no proxy or TLS bypass for other hosts.
    /// # Errors
    /// Rejects zero ports and malformed or excessive origin-form targets.
    #[cfg(any(test, feature = "boundary-fixtures"))]
    pub fn loopback(port: u16, target: &'a str) -> Result<Self, InvalidRequest> {
        validate("127.0.0.1", port, target)?;
        Ok(Self { host: "127.0.0.1", port, target, secure: false })
    }
}
/// Reject authority injection and normalization so the native target preserves the supplied components.
fn validate(host: &str, port: u16, target: &str) -> Result<(), InvalidRequest> {
    if host.is_empty()
        || port == 0
        || host.len() + target.len() > 8192
        || !target.starts_with('/')
        || host.chars().chain(target.chars()).any(char::is_control)
        || host.chars().chain(target.chars()).any(char::is_whitespace)
        || target.contains('#')
    {
        return Err(InvalidRequest);
    }
    let address =
        Url::parse(&format!("https://{host}:{port}{target}")).map_err(|_| InvalidRequest)?;
    if address.host_str() != Some(host)
        || address.port_or_known_default() != Some(port)
        || !address.username().is_empty()
        || address.password().is_some()
        || address.fragment().is_some()
        || &address[url::Position::BeforePath..url::Position::AfterQuery] != target
    {
        return Err(InvalidRequest);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_authority_credentials_controls_and_fragments_without_changing_valid_targets()
    -> Result<(), InvalidRequest> {
        for (host, target) in [
            ("user@example.invalid", "/metadata"),
            ("example.invalid/other", "/metadata"),
            ("user:pass@example.invalid", "/metadata"),
            ("example.invalid", "/x\r\nInjected: yes"),
            ("example.invalid", "/metadata#fragment"),
            ("example.invalid", "http://foreign.invalid/"),
        ] {
            assert!(UpdateRequest::https(host, 443, target).is_err());
        }
        for (host, target) in [
            ("example.invalid", "/package?signature=private%2Fvalue"),
            ("example.invalid", "//metadata"),
            ("[::1]", "/metadata"),
            ("127.0.0.1", "/metadata"),
        ] {
            let request = UpdateRequest::https(host, 8443, target)?;
            assert_eq!(request.target, target);
            assert!(request.secure);
        }
        assert!(!UpdateRequest::loopback(1, "/metadata")?.secure);
        Ok(())
    }
}
