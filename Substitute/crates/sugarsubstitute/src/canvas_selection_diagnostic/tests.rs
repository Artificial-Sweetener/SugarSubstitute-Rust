// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual formatter bytes and strict closed fields of the relayed selection record.
use super::*;
use std::{
    error::Error,
    io::Write,
    sync::{Arc, Mutex},
};

const PREFIX: &str =
    "2026-11-23T20:00:00.000000Z  INFO sugarsubstitute::canvas_selection_diagnostic: ";

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
fn selection_records_survive_the_child_formatter_and_relay() -> Result<(), Box<dyn Error>> {
    for record in [
        Record { selection: 4, route: Route::Retained, tier: Tier::Detail, load: 0, shown: 9_000 },
        Record {
            selection: u64::MAX,
            route: Route::Loaded,
            tier: Tier::Navigation,
            load: u64::MAX,
            shown: u64::MAX,
        },
        Record { selection: 1, route: Route::Loaded, tier: Tier::Fallback, load: 1, shown: 2 },
    ] {
        let line = formatted(record)?;
        assert!(line.len() <= 512, "{line}");
        assert_eq!(Record::parse(line.as_bytes()), Some(record), "{line}");
    }
    Ok(())
}

#[test]
fn a_watch_records_each_tier_once_for_its_own_selection() {
    let ms = Duration::from_millis;
    let mut watch = SelectionWatch::start(3, ms(100));
    watch.loaded(2, ms(150));
    watch.loaded(3, ms(160));
    assert_eq!(watch.shown(2, Tier::Detail, ms(170)), None);
    let first = watch.shown(3, Tier::Fallback, ms(180));
    assert_eq!(
        first,
        Some(Record {
            selection: 3,
            route: Route::Loaded,
            tier: Tier::Fallback,
            load: 60_000,
            shown: 80_000
        })
    );
    assert_eq!(watch.shown(3, Tier::Fallback, ms(190)), None);
    let mut retained = SelectionWatch::start(5, ms(0));
    retained.retained();
    let detail = retained.shown(5, Tier::Detail, ms(7));
    assert_eq!(detail.map(|record| (record.route, record.load)), Some((Route::Retained, 0)));
}

#[test]
fn other_targets_schemas_and_extra_fields_are_refused() {
    let good = "operation=\"canvas_selection_shown\" schema=1 selection=3 route=Loaded \
                tier=Detail load_us=1 shown_us=2";
    assert!(Record::parse(format!("{PREFIX}{good}").as_bytes()).is_some());
    for line in [
        format!("2026  INFO sugarsubstitute::other: {good}"),
        format!("{PREFIX}{}", good.replace("schema=1", "schema=2")),
        format!("{PREFIX}{good} extra=1"),
        format!("{PREFIX}{}", good.replace("route=Loaded", "route=Cached")),
        format!("{PREFIX}{}", good.replace("tier=Detail", "tier=Other")),
        format!("{PREFIX}{}", good.replace("selection=3", "selection=03")),
    ] {
        assert_eq!(Record::parse(line.as_bytes()), None, "{line}");
    }
}
