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
    inline(context, payload, None)?.ok_or_else(exceeded)
}

/// Encode one native command that hands `first` and then `second` to the page, in that order.
///
/// One evaluation delivers both, so no other host message can arrive between them.
/// # Errors
/// Rejects non-UTF-8 payloads and final parameters exceeding native admission.
pub(super) fn encode_ordered(
    context: i64,
    first: &[u8],
    second: &[u8],
) -> Result<String, DesktopError> {
    let utf8 = |payload| {
        std::str::from_utf8(payload).map_err(|cause| DesktopError::new("encode_panel_utf8", cause))
    };
    let expression = format!("{PREFIX}{}),{PREFIX}{})", utf8(first)?, utf8(second)?);
    let parameters = encode_parameters(context, &expression)?;
    if parameters.len() > MAX_PARAMETERS_BYTES {
        return Err(exceeded());
    }
    Ok(parameters)
}

/// Qualify an ordinary response with the exact private ticket that admitted its request.
pub(super) fn encode_owned(
    context: i64,
    payload: &[u8],
    ticket: crate::main_panel_ingress::Ticket,
) -> Result<String, DesktopError> {
    inline(context, payload, Some(ticket))?.ok_or_else(exceeded)
}

/// Encode inline when the final command admits it; `None` hands the bytes to the bulk owner.
///
/// The optional ticket qualifies an ordinary response with its private admission identity.
/// # Errors
/// Rejects non-UTF-8 payloads; size alone never fails here.
pub(super) fn inline(
    context: i64,
    payload: &[u8],
    ticket: Option<crate::main_panel_ingress::Ticket>,
) -> Result<Option<String>, DesktopError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Ok(None);
    }
    let payload = std::str::from_utf8(payload)
        .map_err(|cause| DesktopError::new("encode_panel_utf8", cause))?;
    let expression = match ticket {
        Some(ticket) => format!("{PREFIX}{payload},{})", ticket.0),
        None => format!("{PREFIX}{payload})"),
    };
    let parameters = encode_parameters(context, &expression)?;
    Ok((parameters.len() <= MAX_PARAMETERS_BYTES).then_some(parameters))
}

/// Serialize one bounded private control operation through the same native CDP owner.
///
/// An ordinary-lane grant also carries the upload token for that credit's bulk body.
pub(super) fn transport(
    context: i64,
    action: &str,
    lane: crate::main_panel_ingress::Lane,
    ticket: crate::main_panel_ingress::Ticket,
    upload: Option<crate::main_bulk::Token>,
) -> Result<String, DesktopError> {
    let upload = upload.map(|token| format!(",{:?}", token.hex())).unwrap_or_default();
    let parameters = encode_parameters(
        context,
        &format!(
            "globalThis.__sugarcubes_substitute_transport_v1({action:?},{:?},{}{upload})",
            lane.token(),
            ticket.0
        ),
    )?;
    if parameters.len() > MAX_PARAMETERS_BYTES {
        return Err(exceeded());
    }
    Ok(parameters)
}

/// Report a final command that exceeds native admission.
fn exceeded() -> DesktopError {
    DesktopError::message("encode_panel_delivery", "command input bound exceeded")
}

/// Serialize the fixed `Runtime.evaluate` parameters around one expression.
fn encode_parameters(context: i64, expression: &str) -> Result<String, DesktopError> {
    serde_json::to_string(&Parameters { expression, context_id: context, return_by_value: true })
        .map_err(|cause| DesktopError::new("encode_panel_delivery", cause))
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;
