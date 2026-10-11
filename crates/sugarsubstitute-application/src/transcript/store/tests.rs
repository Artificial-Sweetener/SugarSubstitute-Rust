// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise checked private numeric exhaustion without public test bypasses.
use super::*;
use crate::transcript::tests::{TestResult, batch, feed, resource, text};
#[test]
fn revision_and_record_identity_exhaustion_reject_before_parser_mutation() -> TestResult {
    for record_overflow in [false, true] {
        let mut store = TranscriptStore::new(resource(1)?);
        feed(&mut store, OutputStream::Stdout, 1, b"kept\xe6", false)?;
        if record_overflow {
            store.next_id = u64::MAX;
        } else {
            store.revision = u64::MAX;
        }
        let before =
            (store.revision(), store.retained_counts(), store.diagnostics(), text(&store)?);
        let input = batch(&store, OutputStream::Stdout, 2, b"bad", false)?;
        assert_eq!(store.apply(&input), Err(TranscriptError::Exhausted));
        assert_eq!(
            (store.revision(), store.retained_counts(), store.diagnostics(), text(&store)?),
            before
        );
        if record_overflow {
            store.next_id = 2;
        } else {
            store.revision = 1;
        }
        feed(&mut store, OutputStream::Stdout, 2, b"\x97\xa5", true)?;
        assert_eq!(text(&store)?, vec!["kept日"]);
    }
    Ok(())
}
#[test]
fn stream_sequence_exhaustion_rejects_without_mutating_other_stream() -> TestResult {
    let mut store = TranscriptStore::new(resource(1)?);
    store.streams[0].sequence = u64::MAX;
    assert_eq!(
        feed(&mut store, OutputStream::Stdout, 1, b"bad", true),
        Err(TranscriptError::Exhausted)
    );
    feed(&mut store, OutputStream::Stderr, 1, b"other", true)?;
    assert_eq!(text(&store)?, vec!["other"]);
    Ok(())
}
