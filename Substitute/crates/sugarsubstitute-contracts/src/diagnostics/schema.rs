// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the immutable Rust/worker routing contract and generate quoted constant data.
use serde::Deserialize;
use std::{error::Error, fmt::Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Schema {
    schema_version: u32,
    event_bytes: usize,
    field_bytes: usize,
    queue_events: usize,
    batch_events: usize,
    page_bytes: usize,
    history_pages: usize,
    worker_line_bytes: usize,
    worker_lines: usize,
    identity_bytes: usize,
    operation_bytes: usize,
    sensitive_field_patterns: Vec<String>,
    sensitive_field_names: Vec<String>,
    sensitive_text: Vec<String>,
    closed_code_fields: Vec<String>,
}
impl Schema {
    /// Reject malformed, unbounded or weakened source contracts before either consumer is built.
    pub(crate) fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        if text.len() > 16384 {
            return Err("Diagnostic contract exceeds admission".into());
        }
        let schema: Self = serde_json::from_str(text)?;
        let bounds = [
            (schema.event_bytes, 8192),
            (schema.field_bytes, 2048),
            (schema.queue_events, 64),
            (schema.batch_events, 3),
            (schema.page_bytes, 65536),
            (schema.history_pages, 8),
            (schema.worker_line_bytes, 4096),
            (schema.worker_lines, 8),
            (schema.identity_bytes, 80),
            (schema.operation_bytes, 128),
        ];
        if schema.schema_version != 1
            || bounds.iter().any(|(value, max)| *value == 0 || value > max)
            || schema.batch_events * schema.event_bytes * 2 + 1024 > schema.page_bytes
        {
            return Err("Diagnostic contract has invalid bounds".into());
        }
        for terms in [
            &schema.sensitive_field_patterns,
            &schema.sensitive_field_names,
            &schema.sensitive_text,
            &schema.closed_code_fields,
        ] {
            if terms.is_empty()
                || terms.len() > 32
                || terms.iter().enumerate().any(|(index, term)| {
                    term.is_empty()
                        || term.len() > 64
                        || !term.is_ascii()
                        || term.chars().any(|ch| ch.is_control() || ch.is_ascii_uppercase())
                        || terms[..index].contains(term)
                })
            {
                return Err("Diagnostic contract has invalid privacy terms".into());
            }
        }
        for required in ["password", "token", "secret", "credential", "authorization", "cookie"] {
            if !schema.sensitive_text.iter().any(|term| term == required)
                || !schema.sensitive_field_patterns.iter().any(|term| term == required)
            {
                return Err("Diagnostic contract weakens credential privacy".into());
            }
        }
        if schema.closed_code_fields != ["operation", "native_stage"] {
            return Err("Diagnostic contract changes closed-code exceptions".into());
        }
        Ok(schema)
    }
    /// Produce only integer literals and Rust-quoted immutable strings, never executable input.
    pub(crate) fn rust(&self) -> Result<String, std::fmt::Error> {
        let mut output = String::new();
        writeln!(
            output,
            "/// Current operational-page and diagnostic grammar version.\npub const OPERATIONAL_DIAGNOSTIC_SCHEMA: u32 = {};",
            self.schema_version
        )?;
        for (name, value) in [
            ("EVENT_BYTES", self.event_bytes),
            ("FIELD_BYTES", self.field_bytes),
            ("QUEUE_EVENTS", self.queue_events),
            ("BATCH_EVENTS", self.batch_events),
            ("PAGE_BYTES", self.page_bytes),
            ("HISTORY_PAGES", self.history_pages),
            ("WORKER_LINE_BYTES", self.worker_line_bytes),
            ("WORKER_LINES", self.worker_lines),
            ("IDENTITY_BYTES", self.identity_bytes),
            ("OPERATION_BYTES", self.operation_bytes),
        ] {
            writeln!(
                output,
                "/// Maximum admitted count or UTF-8 byte size for this diagnostic boundary.\npub const DIAGNOSTIC_{name}: usize = {value};"
            )?;
        }
        for (name, values) in [
            ("SENSITIVE_FIELD_PATTERNS", &self.sensitive_field_patterns),
            ("SENSITIVE_FIELD_NAMES", &self.sensitive_field_names),
            ("SENSITIVE_TEXT", &self.sensitive_text),
            ("CLOSED_CODE_FIELDS", &self.closed_code_fields),
        ] {
            writeln!(
                output,
                "/// Canonical privacy projection shared with the provisioning worker.\npub const DIAGNOSTIC_{name}: &[&str] = &{values:?};"
            )?;
        }
        Ok(output)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const CURRENT: &str = include_str!("current.json");
    #[test]
    fn validates_source_contract_and_rejects_unknown_schema_and_unbounded_pages()
    -> Result<(), Box<dyn Error>> {
        let schema = Schema::parse(CURRENT)?;
        assert!(schema.rust()?.contains("DIAGNOSTIC_BATCH_EVENTS: usize = 3"));
        for invalid in [
            CURRENT.replace("\"schema_version\": 1", "\"schema_version\": 2"),
            CURRENT.replace("\"batch_events\": 3", "\"batch_events\": 4"),
            CURRENT.replace("\"page_bytes\": 65536", "\"page_bytes\": 1"),
            CURRENT.replace("\"schema_version\"", "\"unknown\""),
        ] {
            assert!(Schema::parse(&invalid).is_err());
        }
        Ok(())
    }
    #[test]
    fn rejects_weakened_privacy_floor_and_duplicate_terms() {
        assert!(Schema::parse(&CURRENT.replace("\"password\"", "\"other\"")).is_err());
        assert!(Schema::parse(&CURRENT.replace("\"api_key\"", "\"password\"")).is_err());
    }
}
