// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual formatter bytes and strict closed fields of the relayed zoom record.
use super::*;
use std::{
    error::Error,
    io::Write,
    sync::{Arc, Mutex},
};

const PREFIX: &str = "2026-11-22T20:00:00.000000Z  INFO sugarsubstitute::canvas_zoom_diagnostic: ";

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
fn each_snap_and_a_first_step_survive_the_child_formatter_and_relay() -> Result<(), Box<dyn Error>>
{
    for record in [
        Record { gap: None, duration: 80_000, snap: Snap::Animated },
        Record { gap: Some(10_000), duration: 96_000, snap: Snap::Animated },
        Record { gap: Some(400_000), duration: 0, snap: Snap::Unchanged },
        Record { gap: Some(0), duration: 0, snap: Snap::Unchanged },
        Record { gap: Some(u64::MAX), duration: u64::MAX, snap: Snap::Animated },
    ] {
        let line = formatted(record)?;
        assert!(line.len() <= 512, "{line}");
        assert_eq!(Record::parse(line.as_bytes()), Some(record), "{line}");
    }
    Ok(())
}

#[test]
fn other_targets_schemas_and_extra_fields_are_refused() {
    let good = "operation=\"canvas_zoom\" schema=1 gap_us=none duration_us=80000 snap=Animated";
    assert!(Record::parse(format!("{PREFIX}{good}").as_bytes()).is_some());
    for line in [
        format!("2026  INFO sugarsubstitute::other: {good}"),
        format!("{PREFIX}{}", good.replace("schema=1", "schema=2")),
        format!("{PREFIX}{good} extra=1"),
        format!("{PREFIX}{}", good.replace("80000", "-1")),
        format!("{PREFIX}{}", good.replace("80000", "080000")),
        format!("{PREFIX}{}", good.replace("Animated", "Jumped")),
    ] {
        assert_eq!(Record::parse(line.as_bytes()), None, "{line}");
    }
}
