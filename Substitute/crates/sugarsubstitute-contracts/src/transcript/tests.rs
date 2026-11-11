// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate bounded inputs and immutable UTF-8 style coverage.
use super::*;
use crate::lifecycle::*;
use std::error::Error;
fn resource() -> Result<ResourceKey, Box<dyn Error>> {
    Ok(ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([1; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    })
}
#[test]
fn batch_bounds_identity_gap_and_redacted_debug() -> Result<(), Box<dyn Error>> {
    assert_eq!(StreamSequence::new(0), Err(TranscriptError::ZeroIdentity));
    assert_eq!(RecordId::new(0), Err(TranscriptError::ZeroIdentity));
    assert_eq!(OutputGap::new(0, 1), Err(TranscriptError::EmptyGap));
    assert!(
        OutputBatch::new(
            resource()?,
            OutputStream::Stdout,
            StreamSequence::new(1)?,
            &vec![0; MAX_BATCH_BYTES + 1],
            None,
            false
        )
        .is_err()
    );
    let batch = OutputBatch::new(
        resource()?,
        OutputStream::Stdout,
        StreamSequence::new(1)?,
        b"private-secret",
        None,
        false,
    )?;
    assert!(!format!("{batch:?}").contains("private-secret"));
    assert!(
        OutputBatch::new(
            resource()?,
            OutputStream::Stdout,
            StreamSequence::new(1)?,
            &vec![0; MAX_BATCH_BYTES],
            None,
            true
        )
        .is_ok()
    );
    Ok(())
}
#[test]
fn record_ranges_require_exact_utf8_coverage_and_bounds() -> Result<(), Box<dyn Error>> {
    let id = RecordId::new(1)?;
    let style = TextStyle::default();
    assert!(matches!(
        TranscriptRecord::new(
            id,
            OutputStream::Stdout,
            String::with_capacity(MAX_RECORD_BYTES + 1),
            vec![],
            false
        ),
        Err(TranscriptError::Bound)
    ));
    assert!(matches!(
        TranscriptRecord::new(
            id,
            OutputStream::Stdout,
            String::new(),
            Vec::with_capacity(MAX_RECORD_RUNS + 1),
            false
        ),
        Err(TranscriptError::Bound)
    ));
    for runs in [
        vec![],
        vec![StyleRun { start: 0, end: 1, style }],
        vec![StyleRun { start: 1, end: 3, style }],
        vec![StyleRun { start: 0, end: 4, style }],
    ] {
        assert!(TranscriptRecord::new(id, OutputStream::Stderr, "日".into(), runs, true).is_err());
    }
    let record = TranscriptRecord::new(
        id,
        OutputStream::Stderr,
        "日".into(),
        vec![StyleRun { start: 0, end: 3, style }],
        true,
    )?;
    assert_eq!(record.text(), "日");
    assert!(record.allocated_bytes() >= 3 + size_of::<StyleRun>());
    assert!(!format!("{record:?}").contains('日'));
    assert!(
        TranscriptRecord::new(
            id,
            OutputStream::Stdout,
            "x".repeat(MAX_RECORD_BYTES + 1),
            vec![],
            false
        )
        .is_err()
    );
    Ok(())
}
