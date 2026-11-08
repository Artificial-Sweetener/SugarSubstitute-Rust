// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual formatter bytes, strict closed fields and the unchanged native relay bound.
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
        request_id: 25,
        intent: 8,
        current: 8,
        snapshot: NativePreparationSnapshot {
            phase: Phase::Displaying,
            flags: FLAGS,
            scene_source: 8,
            buffers: 1,
            render_id: 34,
            render: Work::Terminal,
            display_id: 35,
            display: Work::Queued,
            pacer: Pacer::Preparing,
        },
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
fn actual_closed_preparation_records_roundtrip_including_largest_native_identities()
-> Result<(), Box<dyn Error>> {
    for phase in Phase::ALL {
        for pacer in Pacer::ALL {
            let value = Record {
                snapshot: NativePreparationSnapshot {
                    phase: *phase,
                    pacer: *pacer,
                    ..record().snapshot
                },
                ..record()
            };
            let line = formatted(value)?;
            assert!(line.len() <= 512);
            assert_eq!(Record::parse(line.as_bytes()), Some(value));
        }
    }
    for work in Work::ALL {
        let value = Record {
            request_id: u64::MAX,
            intent: u64::MAX,
            current: u64::MAX,
            snapshot: NativePreparationSnapshot {
                render_id: if *work == Work::None { 0 } else { u64::MAX },
                render: *work,
                display_id: u64::MAX,
                scene_source: u64::MAX,
                buffers: 2,
                ..record().snapshot
            },
        };
        let line = formatted(value)?;
        assert!(line.len() <= 512, "{line}");
        assert_eq!(Record::parse(line.as_bytes()), Some(value));
    }
    Ok(())
}
#[test]
fn malformed_preparation_states_fields_identities_and_oversized_lines_are_opaque()
-> Result<(), Box<dyn Error>> {
    let valid = formatted(record())?;
    for (from, to) in [
        ("phase=Displaying", "phase=Invented"),
        ("pacer=Preparing", "pacer=Injected"),
        ("render=Terminal", "render=Stopped"),
        ("buffers=1", "buffers=3"),
        ("schema=1", "schema=2"),
        ("request_id=25", "request_id=0"),
        ("intent=8", "intent=08"),
        ("current=8", "current=-8"),
        ("render_id=34", "render_id=0"),
        ("display_id=35", "display_id=18446744073709551616"),
        ("flags=268435455", "flags=268435456"),
        (
            "sugarsubstitute::canvas_preparation_diagnostic:",
            "foreign::canvas_preparation_diagnostic:",
        ),
    ] {
        assert!(Record::parse(valid.replace(from, to).as_bytes()).is_none(), "{from}");
    }
    for invalid in [
        format!("{valid} extra=true"),
        format!("{valid} flags=1"),
        valid.replace(" pacer=Preparing", ""),
        format!("{}{}", "x".repeat(513), valid),
    ] {
        assert!(Record::parse(invalid.as_bytes()).is_none());
    }
    Ok(())
}
