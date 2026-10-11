// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove admission is atomic and malformed escapes stay bounded.
use super::*;
#[test]
fn interrupted_three_byte_unicode_never_bridges_ansi_at_any_batch_split() -> TestResult {
    let bytes = b"\xe6\x1b[31m\x97\xa5";
    for split in 0..=bytes.len() {
        let mut store = TranscriptStore::new(resource(1)?);
        feed(&mut store, OutputStream::Stdout, 1, &bytes[..split], false)?;
        feed(&mut store, OutputStream::Stdout, 2, &bytes[split..], true)?;
        let view = store.snapshot(None, 1)?;
        let record = view.records().next().ok_or("record")?;
        assert_eq!(record.text(), "\u{fffd}\u{fffd}\u{fffd}");
        assert_eq!(record.runs().len(), 2);
        assert_eq!(record.runs()[0].style, TextStyle::default());
        assert_eq!(record.runs()[1].style.foreground, AnsiColor::Indexed(1));
        assert_eq!(store.diagnostics().unicode_replacements, 3);
    }
    Ok(())
}
#[test]
fn raw_escape_interrupts_utf8_and_single_byte_sgr_batches_preserve_colors() -> TestResult {
    let bytes = b"\xc3\x1b[1;93;44m\xa9X\x1b[38;5;201;48;2;1;2;3mY\x1b[0mZ";
    let mut store = TranscriptStore::new(resource(1)?);
    for (index, byte) in bytes.iter().enumerate() {
        feed(
            &mut store,
            OutputStream::Stdout,
            index as u64 + 1,
            std::slice::from_ref(byte),
            index + 1 == bytes.len(),
        )?;
    }
    let view = store.snapshot(None, 1)?;
    let record = view.records().next().ok_or("record")?;
    assert_eq!(record.text(), "\u{fffd}\u{fffd}XYZ");
    assert_eq!(record.runs()[1].style.foreground, AnsiColor::Indexed(11));
    assert_eq!(record.runs()[1].style.background, AnsiColor::Indexed(4));
    assert!(record.runs()[1].style.bold);
    assert_eq!(record.runs()[2].style.foreground, AnsiColor::Indexed(201));
    assert_eq!(record.runs()[2].style.background, AnsiColor::Rgb(1, 2, 3));
    assert_eq!(record.runs()[3].style, TextStyle::default());
    assert_eq!(store.diagnostics().unicode_replacements, 2);
    Ok(())
}
#[test]
fn foreign_stale_replayed_skipped_and_post_eof_batches_do_not_mutate() -> TestResult {
    let mut store = TranscriptStore::new(resource(2)?);
    feed(&mut store, OutputStream::Stdout, 1, b"kept", false)?;
    let baseline = (store.revision(), store.retained_counts(), store.diagnostics(), text(&store)?);
    let inputs = [
        OutputBatch::new(
            resource(1)?,
            OutputStream::Stdout,
            StreamSequence::new(2)?,
            b"bad",
            None,
            false,
        )?,
        batch(&store, OutputStream::Stdout, 1, b"bad", false)?,
        batch(&store, OutputStream::Stdout, 3, b"bad", false)?,
        OutputBatch::new(
            store.resource(),
            OutputStream::Stdout,
            StreamSequence::new(2)?,
            b"bad",
            Some(OutputGap::new(10, 1)?),
            false,
        )?,
    ];
    for input in inputs {
        assert!(store.apply(&input).is_err());
        assert_eq!(
            (store.revision(), store.retained_counts(), store.diagnostics(), text(&store)?),
            baseline
        );
    }
    feed(&mut store, OutputStream::Stdout, 2, b"", true)?;
    let revision = store.revision();
    assert_eq!(
        feed(&mut store, OutputStream::Stdout, 3, b"bad", false),
        Err(TranscriptError::StreamEnded)
    );
    assert_eq!(store.revision(), revision);
    Ok(())
}

#[test]
fn overflow_rejection_preserves_parser_and_history() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    feed(&mut store, OutputStream::Stdout, 1, b"prefix\xe6", false)?;
    let too_far = OutputBatch::new(
        store.resource(),
        OutputStream::Stdout,
        StreamSequence::new(2)?,
        b"bad",
        Some(OutputGap::new(1, u64::MAX)?),
        false,
    )?;
    assert_eq!(store.apply(&too_far), Err(TranscriptError::Exhausted));
    feed(&mut store, OutputStream::Stdout, 2, b"\x97\xa5", true)?;
    assert_eq!(text(&store)?, vec!["prefix日"]);
    Ok(())
}

#[test]
fn malformed_ansi_is_atomic_in_style_and_scratch_never_grows() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    let mut bytes = b"\x1b[32mA\x1b[1;38;2;999;1;1mB\x1b[".to_vec();
    bytes.extend(std::iter::repeat_n(b'9', 1000));
    bytes.extend_from_slice(b"mC\x1b[2JD\x1b]title\x07E\x1b[");
    feed(&mut store, OutputStream::Stdout, 1, &bytes, true)?;
    assert_eq!(text(&store)?, vec!["ABCDE"]);
    let view = store.snapshot(None, 1)?;
    let record = view.records().next().ok_or("record")?;
    assert_eq!(record.runs().len(), 1);
    assert_eq!(record.runs()[0].style.foreground, AnsiColor::Indexed(2));
    assert!(!record.runs()[0].style.bold);
    assert_eq!(store.diagnostics().malformed_escapes, 3);
    for prefix in [b"\x1b[".as_slice(), b"\x1b]".as_slice()] {
        let mut unfinished = prefix.to_vec();
        unfinished.extend(std::iter::repeat_n(b'9', 100));
        let mut store = TranscriptStore::new(resource(1)?);
        feed(&mut store, OutputStream::Stdout, 1, &unfinished, true)?;
        assert_eq!(store.diagnostics().malformed_escapes, 1);
    }
    Ok(())
}

#[test]
fn loss_counters_saturate_explicitly_without_wrapping() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    for sequence in 1..=2 {
        store.apply(&OutputBatch::new(
            store.resource(),
            OutputStream::Stdout,
            StreamSequence::new(sequence)?,
            b"",
            Some(OutputGap::new(u64::MAX, 0)?),
            false,
        )?)?;
    }
    assert_eq!(store.diagnostics().dropped_bytes[0], u64::MAX);
    assert!(store.diagnostics().counter_saturated);
    Ok(())
}
