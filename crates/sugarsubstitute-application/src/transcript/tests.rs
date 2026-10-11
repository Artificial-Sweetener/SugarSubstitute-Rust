// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real incremental parsing, retention and immutable snapshot ownership.
mod boundaries;
mod retention;
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{lifecycle::*, transcript::*};
pub(super) type TestResult = Result<(), Box<dyn Error>>;
pub(super) fn resource(generation: u64) -> Result<ResourceKey, Box<dyn Error>> {
    Ok(ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(generation)?,
    })
}
pub(super) fn batch(
    store: &TranscriptStore,
    stream: OutputStream,
    sequence: u64,
    bytes: &[u8],
    end: bool,
) -> Result<OutputBatch, TranscriptError> {
    OutputBatch::new(store.resource(), stream, StreamSequence::new(sequence)?, bytes, None, end)
}
pub(super) fn feed(
    store: &mut TranscriptStore,
    stream: OutputStream,
    sequence: u64,
    bytes: &[u8],
    end: bool,
) -> Result<TranscriptChange, TranscriptError> {
    store.apply(&batch(store, stream, sequence, bytes, end)?)
}
pub(super) fn text(store: &TranscriptStore) -> Result<Vec<String>, TranscriptError> {
    Ok(store.snapshot(None, MAX_SNAPSHOT_RECORDS)?.records().map(|r| r.text().to_owned()).collect())
}

#[test]
fn every_byte_split_preserves_unicode_and_supported_styles() -> TestResult {
    let input = "日本語\x1b[1;31m彩色😀\x1b[22;38;2;12;34;56m끝\x1b[0m!\n";
    for split in 0..=input.len() {
        let mut store = TranscriptStore::new(resource(1)?);
        feed(&mut store, OutputStream::Stdout, 1, &input.as_bytes()[..split], false)?;
        feed(&mut store, OutputStream::Stdout, 2, &input.as_bytes()[split..], true)?;
        let view = store.snapshot(None, 8)?;
        let records: Vec<_> = view.records().collect();
        assert_eq!(records.len(), 1);
        let record = records[0];
        assert_eq!(record.text(), "日本語彩色😀끝!");
        assert_eq!(record.runs().len(), 4);
        assert_eq!(record.runs()[1].style.foreground, AnsiColor::Indexed(1));
        assert!(record.runs()[1].style.bold);
        assert_eq!(record.runs()[2].style.foreground, AnsiColor::Rgb(12, 34, 56));
        assert!(!record.runs()[2].style.bold);
        assert!(record.complete());
    }
    Ok(())
}

#[test]
fn single_byte_batches_decode_maximal_invalid_subparts_and_eof() -> TestResult {
    let bytes = [0xf0, 0x80, 0x80, 0x80, b'A', 0xed, 0xa0, 0x80, b'B', 0xe2, 0x82];
    let expected = String::from_utf8_lossy(&bytes);
    for split in 0..=bytes.len() {
        let mut store = TranscriptStore::new(resource(1)?);
        feed(&mut store, OutputStream::Stdout, 1, &bytes[..split], false)?;
        feed(&mut store, OutputStream::Stdout, 2, &bytes[split..], true)?;
        assert_eq!(text(&store)?, vec![expected.to_string()]);
        assert_eq!(store.diagnostics().unicode_replacements, 8);
    }
    Ok(())
}

#[test]
fn interleaved_streams_keep_decoder_style_and_progress_independent() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    feed(&mut store, OutputStream::Stdout, 1, b"\x1b[31mold\r\xe6", false)?;
    feed(&mut store, OutputStream::Stderr, 1, b"error\n", false)?;
    feed(&mut store, OutputStream::Stdout, 2, b"\x97\xa5\n", true)?;
    let view = store.snapshot(None, 8)?;
    let records: Vec<_> = view.records().collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].text(), "日");
    assert_eq!(records[0].runs()[0].style.foreground, AnsiColor::Indexed(1));
    assert_eq!(records[1].text(), "error");
    assert_eq!(records[1].runs()[0].style, TextStyle::default());
    Ok(())
}

#[test]
fn crlf_is_one_delimiter_at_every_split_and_cr_keeps_record_identity() -> TestResult {
    for split in 0..=7 {
        let mut store = TranscriptStore::new(resource(1)?);
        feed(&mut store, OutputStream::Stdout, 1, &b"a\r\nb\rc\n"[..split], false)?;
        feed(&mut store, OutputStream::Stdout, 2, &b"a\r\nb\rc\n"[split..], true)?;
        assert_eq!(text(&store)?, vec!["a", "c"]);
    }
    let mut store = TranscriptStore::new(resource(1)?);
    let first = feed(&mut store, OutputStream::Stdout, 1, b"old\r", false)?;
    let id = first.appended.ok_or("expected appended identity")?.0;
    let second = feed(&mut store, OutputStream::Stdout, 2, b"new", false)?;
    assert_eq!(second.replaced[0], Some(id));
    assert_eq!(store.snapshot(None, 1)?.records().next().ok_or("record")?.id(), id);
    assert_eq!(text(&store)?, vec!["new"]);
    Ok(())
}

#[test]
fn gaps_seal_pending_line_reset_only_affected_stream_and_expose_exact_loss() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    feed(&mut store, OutputStream::Stdout, 1, b"prefix\xe6\x1b[", false)?;
    feed(&mut store, OutputStream::Stderr, 1, b"\x1b[32mkeep", false)?;
    let input = OutputBatch::new(
        store.resource(),
        OutputStream::Stdout,
        StreamSequence::new(4)?,
        b"fresh\n",
        Some(OutputGap::new(91, 2)?),
        true,
    )?;
    store.apply(&input)?;
    feed(&mut store, OutputStream::Stderr, 2, b"!", true)?;
    let view = store.snapshot(None, 8)?;
    let records: Vec<_> = view.records().collect();
    assert_eq!(
        records.iter().map(|r| r.text()).collect::<Vec<_>>(),
        vec!["prefix\u{fffd}", "keep!", "fresh"]
    );
    assert_eq!(records[2].runs()[0].style, TextStyle::default());
    assert_eq!(records[1].runs()[0].style.foreground, AnsiColor::Indexed(2));
    assert_eq!(store.diagnostics().dropped_bytes, [91, 0]);
    assert_eq!(store.diagnostics().skipped_batches, [2, 0]);
    Ok(())
}
