// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual formatter output and hostile protocol input cannot broaden diagnostic fields.
use super::*;
use std::{
    error::Error,
    io::Write,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("Fixture sink poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn actual_readiness_formatter_roundtrips_within_the_original_line_bound()
-> Result<(), Box<dyn Error>> {
    let identity = MainIdentity {
        run: crate::production::child::test_support::run()?,
        launch: LaunchId::new(1)?,
        navigation: Generation::new(2)?,
    };
    let record = Record::new(identity);
    let sink = Sink::default();
    let writer = sink.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || record.emit());
    let bytes = sink.0.lock().map_err(|_| "Fixture sink poisoned")?;
    let line = std::str::from_utf8(&bytes)?.trim_end_matches('\n');
    assert!(line.len() < 512);
    assert_eq!(Record::parse(line.as_bytes()), Some(record));
    assert!(!line.contains("namespace"));
    Ok(())
}

#[test]
fn acknowledgement_diagnostic_rejects_unknown_schema_target_shape_and_coordinates() {
    let line = "INFO sugarsubstitute::production::startup_readiness_diagnostic: operation=\"startup_readiness_acknowledged\" schema=1 launch=1 navigation=2";
    assert!(Record::parse(line.as_bytes()).is_some());
    for invalid in [
        line.replace("schema=1", "schema=2"),
        line.replace("launch=1", "launch=0"),
        line.replace("launch=1", "launch=01"),
        line.replace("launch=1", "launch=-1"),
        line.replace("navigation=2", "navigation=18446744073709551616"),
        line.replace("sugarsubstitute::production", "arbitrary::producer"),
        format!("operation=\"unexpected\" {line}"),
        format!("{line} extra=anything"),
        format!("{} {line}", "x".repeat(512)),
    ] {
        assert!(Record::parse(invalid.as_bytes()).is_none());
    }
    assert!(Record::parse(&[255]).is_none());
}
