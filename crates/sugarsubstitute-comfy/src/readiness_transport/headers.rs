// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate complete bounded response framing independently of native transport custody.
use super::{probe::Endpoint, types::ReadinessTransportError};
pub(super) fn parse_headers(
    text: &str,
    endpoint: Endpoint<'_>,
) -> Result<Option<u64>, ReadinessTransportError> {
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
    if encoding.is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
        || transfer.is_some_and(|value| !value.eq_ignore_ascii_case("chunked"))
        || (transfer.is_some() && length.is_some())
    {
        return Err(ReadinessTransportError::invalid("http_response_encoding"));
    }
    if endpoint.json()
        && !content_type.is_some_and(|value| {
            let mut parts = value.split(';');
            parts.next().is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
                && parts.all(|part| part.trim().eq_ignore_ascii_case("charset=utf-8"))
        })
    {
        return Err(ReadinessTransportError::invalid("http_json_content_type"));
    }
    length
        .map(|value| {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ReadinessTransportError::invalid("http_content_length_shape"));
            }
            let value = value
                .parse::<u64>()
                .map_err(|e| ReadinessTransportError::new("http_content_length", e))?;
            if value > endpoint.maximum() {
                return Err(ReadinessTransportError::invalid("http_content_length_bound"));
            }
            Ok(value)
        })
        .transpose()
}
