// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject ambiguous HTTP framing before metadata allocation or resumable file mutation.
use super::Rejection;
use crate::delivery::Response;

/// Bounded selected fields from one final native HTTP response.
pub struct Headers {
    status: u16,
    length: Option<u64>,
    range: Option<String>,
    encoding: Option<String>,
    location: Option<String>,
}
impl Headers {
    /// Parse native raw headers after HTTP transfer decoding, with content decoding disabled.
    /// # Errors
    /// Rejects oversized fields, duplicate framing, folding, conflicting lengths and encodings.
    pub fn parse(status: u16, raw: &str) -> Result<Self, Rejection> {
        if raw.len() > 16 * 1024 {
            return Err(Rejection::Headers);
        }
        let mut lines = raw.split("\r\n");
        let first = lines.next().ok_or(Rejection::Headers)?;
        let mut words = first.split_ascii_whitespace();
        if !matches!(words.next(), Some("HTTP/1.0" | "HTTP/1.1" | "HTTP/2" | "HTTP/3"))
            || words.next().and_then(|v| v.parse::<u16>().ok()) != Some(status)
        {
            return Err(Rejection::Headers);
        }
        let mut result = Self { status, length: None, range: None, encoding: None, location: None };
        let mut transfer = None;
        let mut ended = false;
        for line in lines {
            if line.is_empty() {
                ended = true;
                continue;
            }
            if ended {
                return Err(Rejection::Headers);
            }
            let (name, value) = line.split_once(':').ok_or(Rejection::Headers)?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
                || value.chars().any(|c| c.is_control() && c != '\t')
            {
                return Err(Rejection::Headers);
            }
            let value = value.trim_matches([' ', '\t']);
            match name.to_ascii_lowercase().as_str() {
                "content-length" => {
                    if result.length.is_some()
                        || value.is_empty()
                        || value.len() > 20
                        || !value.bytes().all(|b| b.is_ascii_digit())
                    {
                        return Err(Rejection::Headers);
                    }
                    result.length = Some(value.parse().map_err(|_| Rejection::Headers)?);
                }
                "content-range" => put(&mut result.range, value)?,
                "content-encoding" => put(&mut result.encoding, value)?,
                "location" => put(&mut result.location, value)?,
                "transfer-encoding" => put(&mut transfer, value)?,
                _ => {}
            }
        }
        if result.encoding.as_ref().is_some_and(|v| !v.eq_ignore_ascii_case("identity"))
            || transfer
                .as_ref()
                .is_some_and(|v| !v.eq_ignore_ascii_case("chunked") || result.length.is_some())
        {
            return Err(Rejection::Headers);
        }
        Ok(result)
    }
    /// Final status code, before applying metadata/package-specific policy.
    pub fn status(&self) -> u16 {
        self.status
    }
    /// Optional declared body bytes; actual reads still enforce a separate trusted bound.
    pub fn length(&self) -> Option<u64> {
        self.length
    }
    /// Redirect target, without granting destination authority.
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
    /// Borrow fields for the authenticated package interval validator.
    pub fn package_response(&self) -> Response<'_> {
        Response {
            status: self.status,
            length: self.length,
            range: self.range.as_deref(),
            encoding: self.encoding.as_deref(),
        }
    }
    /// Reject partial or non-success responses for small metadata documents.
    /// # Errors
    /// Rejects unexpected status, Content-Range or declarations exceeding the caller's bound.
    pub fn metadata(&self, maximum: usize) -> Result<(), Rejection> {
        if self.status != 200 {
            return Err(Rejection::Status(self.status));
        }
        if self.range.is_some() || self.length.is_some_and(|n| n > maximum as u64) {
            return Err(Rejection::Headers);
        }
        Ok(())
    }
}
/// Duplicates are rejected even when their values happen to match.
fn put(slot: &mut Option<String>, value: &str) -> Result<(), Rejection> {
    if slot.is_some() || value.len() > 8192 {
        return Err(Rejection::Headers);
    }
    *slot = Some(value.to_owned());
    Ok(())
}
