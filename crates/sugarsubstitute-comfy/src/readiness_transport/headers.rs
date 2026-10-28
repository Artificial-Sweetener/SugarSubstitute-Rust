// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate complete bounded response framing independently of native transport custody.
use super::{probe::Endpoint, types::ReadinessTransportError};
pub(super) fn parse_headers(
    text: &str,
    endpoint: Endpoint<'_>,
) -> Result<Option<u64>, ReadinessTransportError> {
    let headers = Headers::parse(text)?;
    headers.check_encoding()?;
    if endpoint.json()
        && !headers.content_type.is_some_and(|value| {
            let mut parts = value.split(';');
            parts.next().is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
                && parts.all(|part| part.trim().eq_ignore_ascii_case("charset=utf-8"))
        })
    {
        return Err(ReadinessTransportError::invalid("http_json_content_type"));
    }
    headers.length(endpoint.maximum())
}

/// Require the exact bounded PNG size before admitting a cache stream.
pub(crate) fn parse_png_length(text: &str, maximum: u64) -> Result<u64, ReadinessTransportError> {
    let headers = Headers::parse(text)?;
    headers.check_encoding()?;
    if headers.transfer.is_some()
        || !headers.content_type.is_some_and(|value| value.eq_ignore_ascii_case("image/png"))
    {
        return Err(ReadinessTransportError::invalid("http_png_framing"));
    }
    headers
        .length(maximum)?
        .filter(|length| *length >= 33)
        .ok_or_else(|| ReadinessTransportError::invalid("http_png_length"))
}

struct Headers<'a> {
    length: Option<&'a str>,
    content_type: Option<&'a str>,
    transfer: Option<&'a str>,
    encoding: Option<&'a str>,
}

impl<'a> Headers<'a> {
    fn parse(text: &'a str) -> Result<Self, ReadinessTransportError> {
        let mut length = None;
        let mut content_type = None;
        let mut transfer = None;
        let mut encoding = None;
        for line in text.split("\r\n").skip(1).filter(|line| !line.is_empty()) {
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| ReadinessTransportError::invalid("http_header_shape"))?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
                || value.chars().any(|c| c.is_control() && c != '\t')
            {
                return Err(ReadinessTransportError::invalid("http_header_token"));
            }
            let value = value.trim();
            let target = if name.eq_ignore_ascii_case("content-length") {
                Some(&mut length)
            } else if name.eq_ignore_ascii_case("content-type") {
                Some(&mut content_type)
            } else if name.eq_ignore_ascii_case("transfer-encoding") {
                Some(&mut transfer)
            } else if name.eq_ignore_ascii_case("content-encoding") {
                Some(&mut encoding)
            } else {
                None
            };
            if let Some(target) = target
                && target.replace(value).is_some()
            {
                return Err(ReadinessTransportError::invalid("http_duplicate_header"));
            }
        }
        Ok(Self { length, content_type, transfer, encoding })
    }

    fn check_encoding(&self) -> Result<(), ReadinessTransportError> {
        if self.encoding.is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
            || self.transfer.is_some_and(|value| !value.eq_ignore_ascii_case("chunked"))
            || (self.transfer.is_some() && self.length.is_some())
        {
            return Err(ReadinessTransportError::invalid("http_response_encoding"));
        }
        Ok(())
    }

    fn length(&self, maximum: u64) -> Result<Option<u64>, ReadinessTransportError> {
        self.length
            .map(|value| {
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(ReadinessTransportError::invalid("http_content_length_shape"));
                }
                let value = value
                    .parse::<u64>()
                    .map_err(|e| ReadinessTransportError::new("http_content_length", e))?;
                if value > maximum {
                    return Err(ReadinessTransportError::invalid("http_content_length_bound"));
                }
                Ok(value)
            })
            .transpose()
    }
}
