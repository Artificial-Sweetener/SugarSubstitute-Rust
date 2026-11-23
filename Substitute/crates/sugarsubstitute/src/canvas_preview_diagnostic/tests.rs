// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual formatter bytes and strict closed fields of the relayed preview timing record.
use super::*;
use std::{
    error::Error,
    io::Write,
    sync::{Arc, Mutex},
};

const PREFIX: &str =
    "2026-11-22T20:00:00.000000Z  INFO sugarsubstitute::canvas_preview_diagnostic: ";

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

/// The exact line the desktop child's formatter writes for `record`.
fn formatted(record: Record) -> Result<String, Box<dyn Error>> {
    let sink = Sink::default();
    let writer = sink.clone();
    let subscriber = crate::diagnostics::fixture_subscriber(move || writer.clone());
    tracing::subscriber::with_default(subscriber, || record.emit());
    let bytes = sink.0.lock().map_err(|_| "Fixture sink poisoned")?;
    Ok(std::str::from_utf8(&bytes)?.trim_end_matches('\n').to_owned())
}

#[test]
fn offered_frame_timings_survive_the_child_formatter_and_relay() -> Result<(), Box<dyn Error>> {
    for record in [
        Record { token: 1, load: 12_000, prepare: 30_000, offer: 0 },
        Record { token: u64::MAX, load: u64::MAX, prepare: u64::MAX, offer: u64::MAX },
    ] {
        let line = formatted(record)?;
        assert!(line.len() <= 512, "{line}");
        assert_eq!(Record::parse(line.as_bytes()), Some(record), "{line}");
    }
    Ok(())
}

#[test]
fn a_timing_reports_each_step_in_order() {
    let mut timing = PreviewTiming::start();
    timing.loaded();
    timing.prepared();
    let record = timing.offered(7);
    assert_eq!(record.token, 7);
    assert!(record.load < 1_000_000 && record.prepare < 1_000_000 && record.offer < 1_000_000);
}

#[test]
fn other_targets_schemas_and_extra_fields_are_refused() {
    let good =
        "operation=\"canvas_preview_offered\" schema=1 frame=3 load_us=1 prepare_us=2 offer_us=3";
    assert!(Record::parse(format!("{PREFIX}{good}").as_bytes()).is_some());
    for line in [
        format!("2026  INFO sugarsubstitute::other: {good}"),
        format!("{PREFIX}{}", good.replace("schema=1", "schema=2")),
        format!("{PREFIX}{good} extra=1"),
        format!("{PREFIX}{}", good.replace("load_us=1", "load_us=-1")),
        format!("{PREFIX}{}", good.replace("frame=3", "frame=03")),
    ] {
        assert_eq!(Record::parse(line.as_bytes()), None, "{line}");
    }
}
