// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake external shaping while exercising actual projection, row order and original pins.
use super::*;
use sugarsubstitute_application::transcript::TranscriptStore;
use sugarsubstitute_contracts::{lifecycle::*, presentation::console_color, transcript::*};
use sugarsubstitute_ui::text::console::{Background, BackgroundPaint, ConsolePaintSpec};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn store(text: &str) -> Result<TranscriptStore, Box<dyn std::error::Error>> {
    let resource = ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let mut value = TranscriptStore::new(resource);
    for (index, bytes) in text.as_bytes().chunks(MAX_BATCH_BYTES).enumerate() {
        value.apply(&OutputBatch::new(
            resource,
            OutputStream::Stdout,
            StreamSequence::new(index as u64 + 1)?,
            bytes,
            None,
            false,
        )?)?;
    }
    Ok(value)
}

#[test]
fn cooperative_projection_keeps_unicode_and_actual_supported_style_ranges() -> TestResult {
    let original = format!("{}\x1b[1;31;44m😀e\u{301}日本語\x1b[0m!\n", "界".repeat(1400));
    let owner = store(&original)?;
    let snapshot = owner.snapshot_tail(1)?;
    let record = snapshot.records().next().ok_or("Record absent")?;
    let mut projection = Projection::new(None)?;
    assert!(!projection.step(record)?);
    assert!(projection.offset <= 4096);
    assert!(record.text().is_char_boundary(projection.offset));
    assert!(projection.step(record)?);
    assert!(!projection.clipped);
    assert_eq!(projection.text, record.text());
    assert_eq!(projection.runs.len(), 3);
    let styled = &projection.runs[1];
    assert!(styled.style.bold);
    assert_eq!(styled.style.foreground, AnsiColor::Indexed(1));
    assert_eq!(styled.style.background, AnsiColor::Indexed(4));
    assert_eq!(&projection.text[styled.start as usize..styled.end as usize], "😀e\u{301}日本語");
    assert_eq!(styled.utf16_end - styled.utf16_start, 7);
    assert_eq!(record.text().chars().count(), 1407);
    Ok(())
}

#[test]
fn drawing_bounds_clip_without_modifying_complete_original_record() -> TestResult {
    for original in [
        "😀".repeat(3000),
        (0..300)
            .map(|i| format!("\x1b[{}mx", if i % 2 == 0 { 31 } else { 32 }))
            .collect::<String>(),
    ] {
        let owner = store(&original)?;
        let view = owner.snapshot_tail(1)?;
        let record = view.records().next().ok_or("Record absent")?;
        let length = record.text().len();
        let styles = record.runs().len();
        let mut projection = Projection::new(None)?;
        while !projection.step(record)? {}
        assert!(projection.clipped);
        assert!(projection.utf16 <= 4096);
        assert!(projection.runs.len() <= 256);
        assert_eq!(record.text().len(), length);
        assert_eq!(record.runs().len(), styles);
        assert!(record.text().starts_with(&projection.text));
    }
    Ok(())
}

struct Paint(BackgroundPaint);
impl ConsoleGlyph for Paint {
    fn step(&mut self, target: &mut Bitmap, max: NonZeroUsize) -> io::Result<bool> {
        assert_eq!(max.get(), 32768);
        self.0.step(target, max)
    }
}
#[derive(Default)]
struct Fonts {
    rows: Vec<(String, usize, f32)>,
}
impl ConsoleFont for Fonts {
    type Paint = Paint;
    fn prepare_console(
        &mut self,
        row: ConsoleRowView<'_>,
        spec: ConsolePaintSpec,
        _: Option<MemoryLease>,
    ) -> io::Result<Paint> {
        assert!(!row.runs.is_empty());
        self.rows.push((row.text.to_owned(), self.rows.len(), spec.top()));
        // Only external shaped geometry is fake; the clipping/composition owner is real.
        let spans = vec![Background {
            bounds: [28.0, spec.top(), 34.0, spec.top() + 1.0],
            color: console_color(row.runs[0].style.foreground, Theme::Dark, false)
                .ok_or_else(|| io::Error::other("Foreground absent"))?,
        }];
        Ok(Paint(BackgroundPaint::new(spans, spec)?))
    }
}

#[test]
fn actual_console_owner_sequences_latest_ten_rows_and_keeps_same_snapshot_credit() -> TestResult {
    let owner = store(&(0..15).map(|i| format!("row{i}\n")).collect::<String>())?;
    let view = owner.snapshot_tail(15)?;
    let others = [owner.snapshot_tail(1)?, owner.snapshot_tail(1)?, owner.snapshot_tail(1)?];
    let mut fonts = Fonts::default();
    let mut cursor = ConsoleRender::<Paint>::new(&view, Theme::Dark, 1.0, 558, 558);
    let mut target = Bitmap::new(558, 558).ok_or("Raster absent")?;
    while !cursor.step(&view, &mut fonts, &mut target, None)? {
        assert!(matches!(owner.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
    }
    assert_eq!(
        fonts.rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        ["row5", "row6", "row7", "row8", "row9", "row10", "row11", "row12", "row13", "row14"]
    );
    assert_eq!(fonts.rows[0].2, 368.0);
    assert_eq!(fonts.rows[9].2, 512.0);
    assert_eq!(cursor.clipped_rows, 0);
    assert_eq!(target.data().chunks_exact(4).filter(|pixel| pixel[3] != 0).count(), 40);
    assert!(target.data().chunks_exact(4).enumerate().all(|(n, pixel)| pixel[3] == 0
        || (30..34).contains(&(n % 558)) && (368..528).contains(&(n / 558))));
    drop(cursor);
    assert!(matches!(owner.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
    drop(view);
    assert!(owner.snapshot_tail(1).is_ok());
    drop(others);
    Ok(())
}

struct FailedFont;
impl ConsoleFont for FailedFont {
    type Paint = Paint;
    fn prepare_console(
        &mut self,
        _: ConsoleRowView<'_>,
        _: ConsolePaintSpec,
        _: Option<MemoryLease>,
    ) -> io::Result<Paint> {
        Err(io::Error::new(io::ErrorKind::NotFound, "External font fixture failure"))
    }
}

#[test]
fn clipped_row_fact_is_actual_bounded_and_survives_original_font_failure() -> TestResult {
    let text = format!("{}\n", "😀".repeat(3000));
    let owner = store(&text.repeat(10))?;
    let view = owner.snapshot_tail(10)?;
    let mut target = Bitmap::new(558, 558).ok_or("Raster absent")?;
    let mut fonts = Fonts::default();
    let mut cursor = ConsoleRender::<Paint>::new(&view, Theme::Dark, 1.0, 558, 558);
    while !cursor.step(&view, &mut fonts, &mut target, None)? {}
    assert_eq!(cursor.clipped_rows(), 10);
    assert!(fonts.rows.iter().all(|row| row.0.encode_utf16().count() == 4096));
    assert!(view.records().all(|record| record.text().chars().count() == 3000));
    let mut cursor = ConsoleRender::<Paint>::new(&view, Theme::Dark, 1.0, 558, 558);
    let mut fonts = FailedFont;
    loop {
        match cursor.step(&view, &mut fonts, &mut target, None) {
            Ok(false) => {}
            Ok(true) => return Err("Failed font unexpectedly completed".into()),
            Err(cause) => {
                assert_eq!(cause.kind(), io::ErrorKind::NotFound);
                assert_eq!(cause.to_string(), "External font fixture failure");
                break;
            }
        }
    }
    assert_eq!(cursor.clipped_rows(), 1);
    assert_eq!(view.records().len(), 10);
    Ok(())
}
