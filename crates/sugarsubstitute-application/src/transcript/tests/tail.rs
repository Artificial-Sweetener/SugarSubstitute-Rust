// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual indexed tail selection, immutable pins and shared admission.

use super::*;
use std::cell::Cell;

fn ids(view: &TranscriptSnapshot) -> Vec<u64> {
    view.records().map(|record| record.id().get()).collect()
}

fn large_line(store: &mut TranscriptStore, sequence: &mut u64) -> TestResult {
    let bytes = vec![b'x'; MAX_BATCH_BYTES];
    for input in [&bytes[..], &bytes[..], &b"\n"[..]] {
        *sequence += 1;
        feed(store, OutputStream::Stdout, *sequence, input, false)?;
    }
    Ok(())
}

#[test]
fn tail_selects_actual_latest_records_and_preserves_anchored_prefix() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    feed(&mut store, OutputStream::Stdout, 1, &vec![b'\n'; 2000], false)?;
    let oldest = store.snapshot(None, 128)?;
    assert_eq!(ids(&oldest), (1..=128).collect::<Vec<_>>());
    let tail = store.snapshot_tail(10)?;
    assert_eq!(ids(&tail), (1991..=2000).collect::<Vec<_>>());
    assert_eq!(tail.resource(), store.resource());
    assert_eq!(tail.revision(), store.revision());
    assert_eq!(tail.diagnostics(), store.diagnostics());
    Ok(())
}

#[test]
fn byte_bound_retains_the_newest_fitting_suffix() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    let mut sequence = 0;
    for _ in 0..8 {
        large_line(&mut store, &mut sequence)?;
    }
    assert_eq!(ids(&store.snapshot(None, 128)?), [1, 2, 3, 4]);
    let tail = store.snapshot_tail(10)?;
    assert_eq!(ids(&tail), [5, 6, 7, 8]);
    assert_eq!(tail.records().map(|record| record.text().len()).sum::<usize>(), MAX_SNAPSHOT_BYTES);
    Ok(())
}

#[test]
fn empty_tail_and_invalid_limits_preserve_shared_credits() -> TestResult {
    let store = TranscriptStore::new(resource(1)?);
    for limit in [0, 129] {
        assert!(matches!(store.snapshot_tail(limit), Err(TranscriptError::Bound)));
    }
    let a = store.snapshot_tail(10)?;
    let b = store.snapshot(None, 10)?;
    let c = store.snapshot_tail(128)?;
    let d = store.snapshot(None, 1)?;
    assert_eq!(a.records().len(), 0);
    assert_eq!(a.revision(), 0);
    assert!(matches!(store.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
    drop(b);
    let replacement = store.snapshot_tail(1)?;
    drop((a, c, d, replacement));
    assert!(store.snapshot_tail(1).is_ok());
    Ok(())
}

#[test]
fn tail_pins_current_cr_versions_in_stable_interleaved_order() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    feed(&mut store, OutputStream::Stdout, 1, b"old\r", false)?;
    feed(&mut store, OutputStream::Stderr, 1, b"error\n", false)?;
    let old = store.snapshot_tail(10)?;
    feed(&mut store, OutputStream::Stdout, 2, "日本語😀".as_bytes(), true)?;
    let current = store.snapshot_tail(10)?;
    assert_eq!(ids(&old), ids(&current));
    assert_eq!(old.records().map(|r| r.text()).collect::<Vec<_>>(), ["old", "error"]);
    assert_eq!(current.records().map(|r| r.text()).collect::<Vec<_>>(), ["日本語😀", "error"]);
    drop(store);
    assert_eq!(current.records().next().ok_or("Tail pin absent")?.text(), "日本語😀");
    Ok(())
}

#[test]
fn trimming_and_store_drop_preserve_tail_pins_and_bound_allocations() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    let mut sequence = 0;
    for _ in 0..130 {
        large_line(&mut store, &mut sequence)?;
    }
    let views = [
        store.snapshot_tail(10)?,
        store.snapshot_tail(10)?,
        store.snapshot_tail(10)?,
        store.snapshot_tail(10)?,
    ];
    let expected = ids(&views[0]);
    for _ in 0..130 {
        large_line(&mut store, &mut sequence)?;
    }
    assert!(store.diagnostics().trimmed_records > 0);
    assert_eq!(ids(&views[0]), expected);
    let bytes = store.allocated_bytes()
        + views.iter().map(TranscriptSnapshot::allocated_bytes).sum::<usize>();
    assert!(bytes <= MAX_OWNER_ALLOCATION_BYTES);
    drop(store);
    assert_eq!(ids(&views[0]), expected);
    Ok(())
}

#[test]
fn actual_indexed_selection_and_pin_iterators_visit_at_most_twice_the_limit() -> TestResult {
    use crate::transcript::snapshot::tail_start;
    let mut full = TranscriptStore::new(resource(1)?);
    feed(&mut full, OutputStream::Stdout, 1, &vec![b'\n'; 2000], false)?;
    let mut large = TranscriptStore::new(resource(2)?);
    let mut sequence = 0;
    for _ in 0..8 {
        large_line(&mut large, &mut sequence)?;
    }
    let empty = TranscriptStore::new(resource(3)?);
    for store in [&full, &large, &empty] {
        for limit in [1, 10, 128] {
            let reverse_visits = Cell::new(0);
            let pin_visits = Cell::new(0);
            let start = tail_start(
                store
                    .records
                    .iter()
                    .enumerate()
                    .rev()
                    .inspect(|_| reverse_visits.set(reverse_visits.get() + 1)),
                store.records.len(),
                limit,
            );
            let instrumented = store.pin_snapshot(
                store.records.range(start..).inspect(|_| pin_visits.set(pin_visits.get() + 1)),
                limit,
            )?;
            let actual = store.snapshot_tail(limit)?;
            assert_eq!(ids(&instrumented), ids(&actual));
            assert!(reverse_visits.get() <= limit);
            assert!(pin_visits.get() <= limit);
            assert!(reverse_visits.get() + pin_visits.get() <= 2 * limit);
            if store.resource() == full.resource() {
                assert_eq!(start, 2000 - limit);
                assert_eq!(reverse_visits.get(), limit);
                assert_eq!(pin_visits.get(), limit);
                assert_eq!(ids(&actual).last(), Some(&2000));
            } else if store.resource() == large.resource() && limit > 4 {
                assert_eq!(reverse_visits.get(), 5);
                assert_eq!(pin_visits.get(), 4);
                assert_eq!(ids(&actual), [5, 6, 7, 8]);
            }
        }
    }
    Ok(())
}
