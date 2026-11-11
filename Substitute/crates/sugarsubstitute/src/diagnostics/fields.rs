// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound formatting before allocation and redact copies at the supported routing boundary.
use std::fmt::{self, Write as _};
use sugarsubstitute_contracts::diagnostics::{
    Correlation, CorrelationKind, DIAGNOSTIC_CLOSED_CODE_FIELDS, DIAGNOSTIC_EVENT_BYTES,
    DIAGNOSTIC_FIELD_BYTES, DIAGNOSTIC_SENSITIVE_FIELD_NAMES, DIAGNOSTIC_SENSITIVE_FIELD_PATTERNS,
    DIAGNOSTIC_SENSITIVE_TEXT, DiagnosticOperation,
};
use tracing::field::{Field, Visit};
use tracing_subscriber::fmt::{FormatFields, format::Writer};

/// Format structured fields without accepting arbitrary credential or filesystem detail.
pub(super) struct Fields;
impl<'writer> FormatFields<'writer> for Fields {
    fn format_fields<R: tracing_subscriber::field::RecordFields>(
        &self,
        writer: Writer<'writer>,
        fields: R,
    ) -> fmt::Result {
        let mut visitor =
            Visitor { writer, first: true, remaining: DIAGNOSTIC_EVENT_BYTES, result: Ok(()) };
        fields.record(&mut visitor);
        visitor.result
    }
}
/// Apply the same bounded privacy projection to every structured field representation.
struct Visitor<'a> {
    writer: Writer<'a>,
    first: bool,
    remaining: usize,
    result: fmt::Result,
}
impl Visitor<'_> {
    /// Bound formatter output before allocation and reject sensitive or malformed copies.
    fn value(&mut self, field: &Field, value: impl fmt::Display) {
        if self.result.is_err() || self.remaining == 0 {
            return;
        }
        let mut text =
            Bounded { text: String::new(), limit: self.remaining.min(DIAGNOSTIC_FIELD_BYTES) };
        let complete = if sensitive(field.name(), "") {
            text.text.push_str("[redacted]");
            true
        } else {
            write!(text, "{value}").is_ok()
        };
        let canonical = text.text.trim_matches('"');
        let invalid_identity = CorrelationKind::from_field(field.name())
            .is_some_and(|kind| Correlation::label(kind, canonical).is_err());
        let invalid_code = DIAGNOSTIC_CLOSED_CODE_FIELDS.contains(&field.name())
            && DiagnosticOperation::new(canonical).is_err();
        let value =
            if !complete || invalid_identity || invalid_code || sensitive(field.name(), &text.text)
            {
                "[redacted]"
            } else {
                &text.text
            };
        if !self.first {
            self.result = self.writer.write_char(' ');
        }
        self.first = false;
        if self.result.is_ok() {
            self.result = if field.name() == "message" {
                self.writer.write_str(value)
            } else {
                write!(self.writer, "{}={value}", field.name())
            };
        }
        self.remaining = self.remaining.saturating_sub(value.len() + field.name().len() + 2);
    }
}
impl Visit for Visitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.value(field, format_args!("{value:?}"));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "operation" && DiagnosticOperation::new(value).is_err() {
            self.value(field, "[invalid operation]");
            return;
        }
        if let Some(kind) = CorrelationKind::from_field(field.name()) {
            match Correlation::label(kind, value) {
                Ok(identity) => self.value(field, format_args!("\"{identity}\"")),
                Err(_) => self.value(field, "[invalid identity]"),
            }
            return;
        }
        self.value(field, format_args!("{value:?}"));
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        if let Some(kind) = CorrelationKind::from_field(field.name()) {
            match Correlation::sequence(kind, value) {
                Ok(identity) => self.value(field, identity),
                Err(_) => self.value(field, "[invalid identity]"),
            }
            return;
        }
        self.value(field, value);
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.value(field, value);
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.value(field, value);
    }
}
/// Refuse overlong values while their formatter is running, before retaining its output.
struct Bounded {
    text: String,
    limit: usize,
}
impl fmt::Write for Bounded {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.limit.saturating_sub(self.text.len()) {
            return Err(fmt::Error);
        }
        self.text.extend(value.chars().map(|ch| if ch.is_control() { ' ' } else { ch }));
        Ok(())
    }
}
/// Free-form errors stay at their authoritative owner; persisted/routed copies omit their content.
pub(super) fn sensitive(field: &str, text: &str) -> bool {
    if DIAGNOSTIC_CLOSED_CODE_FIELDS.contains(&field)
        && DiagnosticOperation::new(text.trim_matches('"')).is_ok()
    {
        return false;
    }
    let name = field.to_ascii_lowercase();
    let value: String = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch.to_ascii_lowercase() })
        .collect();
    DIAGNOSTIC_SENSITIVE_FIELD_PATTERNS.iter().any(|needle| name.contains(needle))
        || DIAGNOSTIC_SENSITIVE_FIELD_NAMES.contains(&field)
        || DIAGNOSTIC_SENSITIVE_TEXT.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensitive_copies_are_rejected_before_routing() {
        for (field, value) in [
            ("cause", "original cause"),
            ("message", r"C:\Users\private"),
            ("message", "Bearer abc"),
            ("message", "Bearer\tabc"),
            ("message", "Bearer\u{0085}abc"),
            ("access_token", "abc"),
            ("url", "host"),
        ] {
            assert!(sensitive(field, value));
        }
        assert!(!sensitive("operation", "startup_readiness_acknowledged"));
        assert!(!sensitive("native_stage", "pipe_open_message_token"));
        assert!(!sensitive("operation", "verify_primary_token"));
        assert!(!sensitive("request_id", "42"));
    }
    #[test]
    fn formatter_stops_before_retaining_oversized_output_and_controls() {
        let mut value = Bounded { text: String::new(), limit: 8 };
        assert!(write!(value, "{}", "x".repeat(9)).is_err());
        assert!(value.text.is_empty());
        assert!(write!(value, "a\nb").is_ok());
        assert_eq!(value.text, "a b");
    }
}
