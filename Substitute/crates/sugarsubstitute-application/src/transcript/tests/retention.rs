// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify text/metadata bounds and snapshot retention through actual store mutations.
use super::*;
#[test]
fn current_and_pinned_old_storage_stay_bounded_across_trims_and_replacements() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    let mut line = vec![b'x'; MAX_BATCH_BYTES - 1];
    line.push(b'\n');
    for sequence in 1..=256 {
        feed(&mut store, OutputStream::Stdout, sequence, &line, false)?;
    }
    let snapshots = [
        store.snapshot(None, 128)?,
        store.snapshot(None, 128)?,
        store.snapshot(None, 128)?,
        store.snapshot(None, 128)?,
    ];
    for sequence in 257..=512 {
        feed(&mut store, OutputStream::Stdout, sequence, &line, false)?;
    }
    let bytes = store.allocated_bytes()
        + snapshots.iter().map(TranscriptSnapshot::allocated_bytes).sum::<usize>();
    assert!(bytes <= MAX_OWNER_ALLOCATION_BYTES);
    assert_eq!(snapshots[0].records().len(), 8);
    assert_eq!(snapshots[0].records().next().ok_or("old record")?.id().get(), 1);
    assert!(store.diagnostics().trimmed_records > 0);
    Ok(())
}
#[test]
fn record_byte_bound_is_utf8_safe_and_cr_allows_fresh_progress() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    let bytes = vec![b'x'; MAX_BATCH_BYTES];
    feed(&mut store, OutputStream::Stdout, 1, &bytes, false)?;
    feed(&mut store, OutputStream::Stdout, 2, &bytes, false)?;
    feed(&mut store, OutputStream::Stdout, 3, "日oversized".as_bytes(), false)?;
    assert_eq!(store.retained_counts().1, MAX_RECORD_BYTES);
    assert_eq!(store.diagnostics().truncated_records, 1);
    assert_eq!(store.diagnostics().truncated_bytes, "日oversized".len() as u64);
    feed(&mut store, OutputStream::Stdout, 4, b"\rfresh\n", true)?;
    assert_eq!(text(&store)?, vec!["fresh"]);
    Ok(())
}

#[test]
fn line_and_text_bounds_trim_oldest_with_stable_anchor_metadata() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    let change = feed(&mut store, OutputStream::Stdout, 1, &vec![b'\n'; MAX_RECORDS + 13], false)?;
    assert_eq!(store.retained_counts().0, MAX_RECORDS);
    assert_eq!(change.trimmed_through.ok_or("trim marker")?.get(), 13);
    assert_eq!(store.diagnostics().trimmed_records, 13);
    let mut large = TranscriptStore::new(resource(1)?);
    let bytes = vec![b'x'; MAX_BATCH_BYTES];
    for line in 0..130 {
        feed(&mut large, OutputStream::Stdout, line * 3 + 1, &bytes, false)?;
        feed(&mut large, OutputStream::Stdout, line * 3 + 2, &bytes, false)?;
        feed(&mut large, OutputStream::Stdout, line * 3 + 3, b"\n", false)?;
    }
    assert_eq!(large.retained_counts().1, MAX_TEXT_BYTES);
    assert_eq!(large.retained_counts().0, 128);
    assert_eq!(large.diagnostics().trimmed_records, 2);
    assert_eq!(large.snapshot(None, 128)?.records().len(), 4);
    Ok(())
}

#[test]
fn snapshot_bounds_and_trimmed_anchor_choose_retained_successor() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    feed(&mut store, OutputStream::Stdout, 1, &vec![b'\n'; MAX_RECORDS + 13], false)?;
    assert!(matches!(store.snapshot(None, 0), Err(TranscriptError::Bound)));
    assert!(matches!(store.snapshot(None, 129), Err(TranscriptError::Bound)));
    let view = store.snapshot(Some(RecordId::new(1)?), 128)?;
    assert_eq!(view.records().len(), 128);
    assert_eq!(view.records().next().ok_or("record")?.id().get(), 14);
    Ok(())
}

#[test]
fn style_run_record_and_history_bounds_are_observable() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    let mut bytes = Vec::new();
    for index in 0..MAX_RECORD_RUNS + 3 {
        bytes.extend_from_slice(if index % 2 == 0 { b"\x1b[31mx" } else { b"\x1b[32mx" });
    }
    bytes.push(b'\n');
    for sequence in 1..=9 {
        feed(&mut store, OutputStream::Stdout, sequence, &bytes, false)?;
    }
    assert_eq!(store.retained_counts().2, MAX_STYLE_RUNS);
    assert_eq!(store.retained_counts().0, 8);
    assert_eq!(store.diagnostics().truncated_records, 9);
    assert_eq!(store.diagnostics().truncated_bytes, 27);
    assert_eq!(store.diagnostics().trimmed_records, 1);
    Ok(())
}

#[test]
fn snapshots_pin_old_versions_share_current_records_and_release_four_credits() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    feed(&mut store, OutputStream::Stdout, 1, b"old\r", false)?;
    let a = store.snapshot(None, 1)?;
    let b = store.snapshot(None, 1)?;
    assert!(std::ptr::eq(a.records().next().ok_or("record")?, b.records().next().ok_or("record")?));
    let c = store.snapshot(None, 1)?;
    let d = store.snapshot(None, 1)?;
    assert!(matches!(store.snapshot(None, 1), Err(TranscriptError::SnapshotCapacity)));
    feed(&mut store, OutputStream::Stdout, 2, b"new", true)?;
    assert_eq!(a.records().next().ok_or("record")?.text(), "old");
    drop(b);
    drop(c);
    drop(d);
    let current = store.snapshot(None, 1)?;
    assert_eq!(current.records().next().ok_or("record")?.text(), "new");
    drop(store);
    assert_eq!(a.records().next().ok_or("record")?.text(), "old");
    Ok(())
}
