// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode owned protocol objects within the native CDP command allocation.

use crate::DesktopError;

const MAX_PAYLOAD_BYTES: usize = 32768;
const MAX_PARAMETERS_BYTES: usize = 8192;
const PREFIX: &str = "globalThis.__sugarcubes_substitute_deliver_v3(";

/// Borrow the single expression allocation while serializing native command metadata.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Parameters<'a> {
    expression: &'a str,
    context_id: i64,
    return_by_value: bool,
}

/// Encode an already schema-validated host message for the independently qualified context.
///
/// Callers must supply a complete JSON object from an owned protocol serializer, never
/// authored JavaScript. Their typed message contracts own schema validation; this adapter
/// borrows those encoded bytes without allocating a second protocol object.
/// Native submission independently enforces the same final UTF-8 command bound.
/// # Errors
/// Rejects oversized or non-UTF-8 payloads and final parameters exceeding native admission.
pub(super) fn encode(context: i64, payload: &[u8]) -> Result<String, DesktopError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(DesktopError::message("encode_panel_delivery", "payload bound exceeded"));
    }
    let payload = std::str::from_utf8(payload)
        .map_err(|cause| DesktopError::new("encode_panel_utf8", cause))?;
    let expression = format!("{PREFIX}{payload})");
    let parameters = serde_json::to_string(&Parameters {
        expression: &expression,
        context_id: context,
        return_by_value: true,
    })
    .map_err(|cause| DesktopError::new("encode_panel_delivery", cause))?;
    if parameters.len() > MAX_PARAMETERS_BYTES {
        return Err(DesktopError::message("encode_panel_delivery", "command input bound exceeded"));
    }
    Ok(parameters)
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;
