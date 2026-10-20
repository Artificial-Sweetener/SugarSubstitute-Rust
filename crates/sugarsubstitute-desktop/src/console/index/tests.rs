// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise exact Unicode/style indexing and copied-document admission without native effects.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    lifecycle::*,
    presentation::{FrameIdentity, FrameShape, FrameValidator, PreparedFrame},
};
pub(crate) type TestResult = Result<(), Box<dyn Error>>;
pub(crate) struct Source {
    pub(crate) key: ResourceKey,
    pub(crate) revision: u64,
    pub(crate) records: Vec<TranscriptRecord>,
}
impl ConsoleText for Source {
    fn resource(&self) -> ResourceKey {
        self.key
    }
    fn revision(&self) -> u64 {
        self.revision
    }
    fn len(&self) -> usize {
        self.records.len()
    }
    fn record(&self, index: usize) -> Option<&TranscriptRecord> {
        self.records.get(index)
    }
}
pub(crate) fn source(revision: u64, lines: &[(u64, &str)]) -> Result<Source, Box<dyn Error>> {
    let key = ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let mut records = Vec::new();
    for (id, text) in lines {
        let runs = if text.is_empty() {
            vec![]
        } else {
            vec![StyleRun {
                start: 0,
                end: text.len() as u32,
                style: TextStyle {
                    foreground: AnsiColor::Rgb(3, 4, 5),
                    background: AnsiColor::Indexed(4),
                    bold: true,
                },
            }]
        };
        records.push(TranscriptRecord::new(
            RecordId::new(*id)?,
            OutputStream::Stdout,
            (*text).to_owned(),
            runs,
            true,
        )?);
    }
    Ok(Source { key, revision, records })
}
struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _bytes: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
pub(crate) fn frame() -> Result<(PreparedFrame, Arc<AtomicUsize>), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    // This boundary fixture models one already-admitted reservation, like native_storage tests.
    let memory =
        MemoryLease::new(NATIVE_CONSOLE_STORAGE_BYTES + 16, Box::new(Release(released.clone())));
    let mut validator = FrameValidator::new(
        FrameShape::new(2, 2, 1.0)?,
        FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        },
        vec![0; 16],
        memory,
    )?;
    assert!(validator.validate_next(NonZeroUsize::MIN.saturating_add(3))?);
    Ok((validator.finish()?, released))
}
pub(crate) fn index(
    source: &dyn ConsoleText,
) -> Result<(DocumentIndex, PreparedFrame, Arc<AtomicUsize>), Box<dyn Error>> {
    let (frame, released) = frame()?;
    let document =
        DocumentIndex::prepare(source, frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)?;
    Ok((document, frame, released))
}
#[test]
fn equal_foreground_runs_share_native_formatting_without_extending_backgrounds() -> TestResult {
    let mut source = source(1, &[(1, "first"), (2, "second")])?;
    let (colored, _, _) = index(&source)?;
    assert_eq!(colored.styles().len(), 2);
    for (slot, text) in source.records.iter_mut().zip(["first", "second"]) {
        *slot = TranscriptRecord::new(
            slot.id(),
            OutputStream::Stdout,
            text.to_owned(),
            vec![StyleRun { start: 0, end: text.len() as u32, style: TextStyle::default() }],
            true,
        )?;
    }
    let (plain, _, _) = index(&source)?;
    assert_eq!(plain.styles().len(), 1);
    assert_eq!((plain.styles()[0].start, plain.styles()[0].end), (0, 12));
    Ok(())
}
#[test]
fn unicode_stream_preserves_exact_foreign_text_and_styles_at_every_callback_boundary() -> TestResult
{
    let source = source(1, &[(1, "a😀日本\t\0"), (2, ""), (3, "e\u{301} 끝")])?;
    let (document, frame, released) = index(&source)?;
    let expected =
        "a😀日本\t\0\r\re\u{301} 끝".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    for size in 1..=19 {
        let mut stream = TextStream::new(&document);
        let mut actual = Vec::new();
        let mut target = vec![0; size];
        loop {
            let length = stream.read(&mut target);
            if length == 0 {
                break;
            }
            actual.extend_from_slice(&target[..length]);
        }
        assert!(stream.complete());
        assert_eq!(actual, expected);
    }
    assert_eq!(document.styles()[0].start, 0);
    assert_eq!(document.styles()[0].end, 7);
    assert_eq!(document.styles()[0].style.foreground, AnsiColor::Rgb(3, 4, 5));
    assert_eq!(source.records[0].text(), "a😀日本\t\0");
    assert!(frame.claim_storage(1).is_err());
    drop(frame);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(document);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    Ok(())
}
#[test]
fn record_anchors_rebase_trimmed_history_and_progress_replacement_without_splitting_surrogates()
-> TestResult {
    let old = source(1, &[(1, "old"), (2, "abcdef"), (3, "tail")])?;
    let (old, _, _) = index(&old)?;
    let anchor = old.anchor(5);
    assert_eq!(anchor, Some(Anchor { record: RecordId::new(2)?, units: 1 }));
    let next = source(2, &[(2, "😀x"), (3, "tail"), (4, "new")])?;
    let (next, _, _) = index(&next)?;
    assert_eq!(next.position(anchor), 0);
    assert_eq!(next.position(old.anchor(0)), 0);
    assert_eq!(next.position(Some(Anchor { record: RecordId::new(2)?, units: 99 })), 3);
    assert_eq!(
        next.position(Some(Anchor { record: RecordId::new(99)?, units: 0 })),
        next.units as i32
    );
    Ok(())
}
#[test]
fn invalid_extent_order_text_and_insufficient_claim_reject_without_native_effects() -> TestResult {
    for lines in [&[(2, "b"), (1, "a")][..], &[(1, "a\nb")], &[(1, "a\rb")]] {
        let source = source(1, lines)?;
        let (frame, _) = frame()?;
        assert!(
            DocumentIndex::prepare(&source, frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)
                .is_err()
        );
        assert!(frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES).is_ok());
    }
    let valid_source = source(1, &[(1, "valid")])?;
    let (frame, _) = frame()?;
    assert!(DocumentIndex::prepare(&valid_source, frame.claim_storage(1)?).is_err());
    let long = "x".repeat(MAX_RECORD_BYTES);
    let source = source(
        1,
        &[(1, long.as_str()), (2, long.as_str()), (3, long.as_str()), (4, long.as_str()), (5, "x")],
    )?;
    assert!(
        DocumentIndex::prepare(&source, frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)
            .is_err()
    );
    Ok(())
}
#[test]
fn maximum_snapshot_copies_only_bounded_text_without_mutating_original_records() -> TestResult {
    let line = "x".repeat(MAX_RECORD_BYTES);
    let source = source(1, &[(1, &line), (2, &line), (3, &line), (4, &line)])?;
    let (document, frame, _) = index(&source)?;
    assert_eq!(document.units as usize, MAX_SNAPSHOT_BYTES + 3);
    assert_eq!(document.text.capacity(), document.units as usize);
    assert_eq!(document.rows.capacity(), 4);
    assert_eq!(document.styles.capacity(), 4);
    drop(document);
    assert!(frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES).is_ok());
    assert_eq!(source.records[0].text(), line);
    Ok(())
}

#[test]
fn empty_native_document_has_no_fabricated_resource_and_keeps_original_charge() -> TestResult {
    let (frame, released) = frame()?;
    let document = DocumentIndex::empty(frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)?;
    assert!(document.resource.is_none());
    assert_eq!(document.units, 0);
    assert!(document.text().is_empty());
    assert!(frame.claim_storage(1).is_err());
    drop(frame);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(document);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    Ok(())
}
