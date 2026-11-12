// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual formatter output round-trips; hostile or foreign lines never broaden relayed fields.
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

fn record() -> Record {
    Record {
        stage: Stage::Waiting,
        operation: Operation::Overview,
        request_id: 115,
        revision: 19,
        generation: 4,
        attachment: Attachment::of("attachment:5321edfd9b3ec7205b5a27aba97a3c75"),
        pending: Pending::Select,
        wait: Wait::Superseded,
        intent: 41,
        current: 42,
        status: Status::None,
    }
}

fn formatted(record: Record) -> Result<String, Box<dyn Error>> {
    let sink = Sink::default();
    let writer = sink.clone();
    let subscriber = crate::diagnostics::fixture_subscriber(move || writer.clone());
    tracing::subscriber::with_default(subscriber, || record.emit());
    let bytes = sink.0.lock().map_err(|_| "Fixture sink poisoned")?;
    Ok(std::str::from_utf8(&bytes)?.trim_end_matches('\n').to_owned())
}

#[test]
fn actual_formatter_output_roundtrips_within_the_relay_line_bound() -> Result<(), Box<dyn Error>> {
    for record in [
        record(),
        Record {
            stage: Stage::Unmatched,
            operation: Operation::Cancel,
            revision: 0,
            generation: 0,
            attachment: None,
            pending: Pending::None,
            wait: Wait::None,
            intent: 0,
            current: 0,
            status: Status::None,
            ..record()
        },
        Record { stage: Stage::Replied, status: Status::Applied, ..record() },
    ] {
        let line = formatted(record)?;
        assert!(line.len() < LINE_BYTES, "{line}");
        assert_eq!(Record::parse(line.as_bytes()), Some(record), "{line}");
        assert!(!line.contains("9b3ec720"), "only the attachment prefix is recorded: {line}");
    }
    Ok(())
}

#[test]
fn foreign_targets_unknown_tokens_and_extra_fields_are_discarded() -> Result<(), Box<dyn Error>> {
    let line = formatted(record())?;
    assert!(Record::parse(line.as_bytes()).is_some());
    for invalid in [
        line.replace("schema=1", "schema=2"),
        line.replace("stage=Waiting", "stage=Unknown"),
        line.replace("op=Overview", "op=overview"),
        line.replace("request_id=115", "request_id=0115"),
        line.replace("revision=19", "revision=-19"),
        line.replace("generation=4", "generation=18446744073709551616"),
        line.replace("\"5321edfd\"", "\"5321EDFD\""),
        line.replace("\"5321edfd\"", "\"C:\\x\""),
        line.replace("wait=Superseded", "wait=superseded"),
        line.replace("sugarsubstitute::canvas_surface_diagnostic", "arbitrary::producer"),
        format!("operation=\"unexpected\" {line}"),
        format!("{line} extra=anything"),
        format!("{} {line}", "x".repeat(LINE_BYTES)),
    ] {
        assert!(Record::parse(invalid.as_bytes()).is_none(), "{invalid}");
    }
    assert!(Record::parse(&[255]).is_none());
    Ok(())
}

#[test]
fn attachments_record_only_a_canonical_hexadecimal_prefix() {
    assert_eq!(
        Attachment::of("attachment:5321edfd9b3ec7205b5a27aba97a3c75")
            .as_ref()
            .map(Attachment::as_str),
        Some("5321edfd")
    );
    for foreign in ["attachment:one", "attachment:5321EDFD9b3e", "view:5321edfd9b3e", "attachment:"]
    {
        assert_eq!(Attachment::of(foreign), None, "{foreign}");
    }
}
