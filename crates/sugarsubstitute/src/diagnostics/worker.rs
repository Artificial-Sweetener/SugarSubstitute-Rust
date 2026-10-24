// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project authenticated worker payloads as untrusted text, independently of trusted host events.
use serde_json::{Value, json};
use sugarsubstitute_contracts::diagnostics::{
    DIAGNOSTIC_WORKER_LINE_BYTES, DIAGNOSTIC_WORKER_LINES,
};

/// Observe a closed event category and the host's own pending request, never worker-asserted correlation.
pub(crate) fn observe(
    event: &str,
    pending: Option<&str>,
) -> Result<(), sugarsubstitute_contracts::diagnostics::DiagnosticIdentityError> {
    let category = match event {
        "progress" => "progress",
        "diagnostic" => "diagnostic",
        "completed" => "completed",
        "failed" => "failed",
        "cancelled" => "cancelled",
        "fatal" => "fatal",
        "protocol_error" => "protocol_error",
        _ => "unknown",
    };
    if let Some(pending) = pending {
        let request = sugarsubstitute_contracts::diagnostics::Correlation::label(
            sugarsubstitute_contracts::diagnostics::CorrelationKind::Request,
            pending,
        )?;
        tracing::debug!(operation="installer_worker_event", event=category, request_id=%request, "Received provisioning event");
    } else {
        tracing::debug!(
            operation = "installer_worker_event",
            event = category,
            "Received provisioning event"
        );
    }
    Ok(())
}

/// Keep the established diagnostic response shape while dropping arbitrary nested worker properties.
/// Text is a redacted presentation copy; it never grants host-event producer trust.
pub(crate) fn project(value: &Value) -> Value {
    let message = value["message"].as_str().unwrap_or("[untrusted diagnostic]");
    let message =
        if message.len() > DIAGNOSTIC_WORKER_LINES * (DIAGNOSTIC_WORKER_LINE_BYTES + 1) - 1 {
            "[oversized diagnostic]".into()
        } else {
            message
                .lines()
                .take(DIAGNOSTIC_WORKER_LINES)
                .map(|line| {
                    if line.len() > DIAGNOSTIC_WORKER_LINE_BYTES {
                        "[oversized diagnostic]".into()
                    } else if super::fields::sensitive("message", line) {
                        "[redacted]".into()
                    } else {
                        line.chars()
                            .map(|ch| if ch.is_control() { ' ' } else { ch })
                            .collect::<String>()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
    let omitted = value["omitted"].as_u64().unwrap_or(0);
    tracing::debug!(
        operation = "worker_diagnostic_received",
        worker_omitted = omitted,
        untrusted_bytes = message.len(),
        "Bounded worker diagnostic received"
    );
    json!({"message":message,"omitted":omitted})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_properties_cannot_inject_trusted_fields_or_sensitive_text() {
        let projected = project(
            &json!({"message":"token=secret", "omitted":5, "operation":"trusted", "properties":{"password":"secret"}}),
        );
        assert_eq!(projected, json!({"message":"[redacted]", "omitted":5}));
        assert_eq!(
            project(&json!({"message":{"nested":"secret"}}))["message"],
            "[untrusted diagnostic]"
        );
    }
    #[test]
    fn worker_text_projection_bounds_lines_bytes_and_controls_without_mutating_the_input() {
        let original = json!({"message":"糖\0text\nnext", "omitted":0});
        assert_eq!(project(&original)["message"], "糖 text\nnext");
        assert_eq!(original["message"], "糖\0text\nnext");
        assert_eq!(
            project(&json!({"message":"x".repeat(32776)}))["message"],
            "[oversized diagnostic]"
        );
        assert_eq!(
            project(&json!({"message":"line\n".repeat(100)}))["message"]
                .as_str()
                .map(|text| text.lines().count()),
            Some(8)
        );
    }
}
