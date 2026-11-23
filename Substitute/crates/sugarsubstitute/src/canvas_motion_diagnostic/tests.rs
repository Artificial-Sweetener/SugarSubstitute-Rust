// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual formatter bytes and strict closed fields of the relayed camera motion record.
use super::*;
use std::{
    error::Error,
    io::Write,
    sync::{Arc, Mutex},
};

const PREFIX: &str =
    "2026-11-23T05:00:00.000000Z  INFO sugarsubstitute::canvas_motion_diagnostic: ";

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
fn motion_records_survive_the_child_formatter_and_relay() -> Result<(), Box<dyn Error>> {
    let smooth = Record {
        elapsed: 86_000,
        period: 6_944,
        first_frame: Some(4_000),
        accepted: 12,
        submitted: 12,
        abandoned: 0,
        skipped: 0,
        longest_gap: 7_100,
        detail: false,
        blocked: [0, 0, 0, 3, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    let missing = Record { first_frame: None, accepted: 0, detail: true, ..smooth };
    let largest = Record {
        elapsed: u32::MAX,
        period: u32::MAX,
        first_frame: Some(u32::MAX),
        accepted: u32::MAX,
        submitted: u32::MAX,
        abandoned: u32::MAX,
        skipped: u32::MAX,
        longest_gap: u32::MAX,
        detail: true,
        // A motion of minutes at every display period still fits the 512-byte relay line.
        blocked: [u16::MAX; 17],
    };
    for record in [smooth, missing, largest] {
        let line = formatted(record)?;
        assert!(line.len() <= 512, "{line}");
        assert_eq!(Record::parse(line.as_bytes()), Some(record), "{line}");
    }
    Ok(())
}

#[test]
fn a_window_becomes_a_record_of_its_own_frames() {
    let mut window = PresentationWindow {
        started: Duration::from_millis(100),
        first_acceptance: Some(Duration::from_millis(104)),
        ..PresentationWindow::default()
    };
    window.stats.projected_accepted = 10;
    window.stats.canonical_accepted = 1;
    window.stats.native_acceptance_cadence.maximum = Duration::from_millis(21);
    let record = Record::new(
        window,
        Duration::from_millis(190),
        Duration::from_micros(6_944),
        false,
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 70_000],
    );
    assert_eq!(record.blocked, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, u16::MAX]);
    assert_eq!(record.elapsed, 90_000);
    assert_eq!(record.first_frame, Some(4_000));
    assert_eq!(record.accepted, 11);
    assert_eq!(record.longest_gap, 21_000);
}

#[test]
fn other_targets_schemas_and_extra_fields_are_refused() {
    let good = "operation=\"canvas_motion\" schema=1 elapsed_us=1 period_us=2 first_frame_us=none accepted=0 submitted=0 abandoned=0 skipped=0 longest_gap_us=0 detail=false blocked=disp:0,gpu:0,cpu:0,pace:0,back:0,same:0,buf:0,ret:0,rest:0,acq:0,init:0,pick:0,ginit:0,route:0,open:0,susp:0,stale:0";
    assert!(Record::parse(format!("{PREFIX}{good}").as_bytes()).is_some());
    for line in [
        format!("2026  INFO sugarsubstitute::other: {good}"),
        format!("{PREFIX}{}", good.replace("schema=1", "schema=2")),
        format!("{PREFIX}{good} extra=1"),
        format!("{PREFIX}{}", good.replace("elapsed_us=1", "elapsed_us=-1")),
        format!("{PREFIX}{}", good.replace("detail=false", "detail=maybe")),
        format!("{PREFIX}{}", good.replace(",stale:0", "")),
        format!("{PREFIX}{}", good.replace("ret:0", "ret:70000")),
        format!("{PREFIX}{}", good.replace("pace:0", "pace:-1")),
    ] {
        assert_eq!(Record::parse(line.as_bytes()), None, "{line}");
    }
}
