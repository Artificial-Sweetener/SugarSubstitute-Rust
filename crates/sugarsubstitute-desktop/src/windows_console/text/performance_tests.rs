// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure retained native console work under continuous output, without displaying any window.
use super::*;
use std::time::Instant;

/// Native client heights need not end on a complete text line.
#[test]
fn native_console_every_viewport_height_preserves_semantic_paint() -> TestResult {
    use windows::Win32::UI::WindowsAndMessaging::{
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos,
    };
    let mut apartment = UiApartment::enter()?;
    let (frame, released) = frame_with_shape(FrameShape::new(558, 558, 1.0)?)?;
    let rows: Vec<_> = (1..=128).map(|id| (id, "Imported dependency; server ready")).collect();
    let source = source(1, &rows)?;
    let document =
        DocumentIndex::prepare(&source, frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)?;
    let mut hidden = Hidden::new(document, &apartment)?;
    let hwnd = hidden.hwnd.ok_or("Native child absent")?;
    let mut documents = Documents::new(hidden.document.take().ok_or("Document absent")?);
    publish(hwnd, &mut documents, true)?;
    let engine = engine::find(hwnd)?;
    let mut storage = None;
    let target = Dib::for_frame(&mut storage, &frame)?;
    for height in (80..=360).step_by(3) {
        // SAFETY: Resize only the retained hidden child, without activation or moving its parent.
        unsafe {
            SetWindowPos(hwnd, None, 0, 0, 500, height, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER)
        }?;
        messages::Messages(hwnd).scroll_bottom()?;
        let crop = RECT { left: 25, top: 20, right: 525, bottom: 20 + height };
        engine.prepare(
            &frame,
            Some(documents.shared().ok_or("Shared document absent")?),
            crop,
            true,
            1.0,
        )?;
        engine
            .qualify_pixels(target.dc(), false, target.pixels_mut()?)
            .map_err(|cause| DesktopError::new("console_variable_height", cause))?;
        assert!(engine.at_bottom()?, "Tail lost at height {height}");
    }
    target.close()?;
    hidden.close()?;
    drop((engine, documents, hidden, frame));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    // SAFETY: All hidden native fixture resources have retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}

/// Long wrapped paragraphs and changing ANSI spans exercise RichEdit's lazy viewport layout.
#[test]
fn native_console_rolling_mixed_styles_and_wrapping_keeps_every_draw_semantic() -> TestResult {
    let mut apartment = UiApartment::enter()?;
    let (frame, released) = frame()?;
    let empty = source(1, &[])?;
    let document =
        DocumentIndex::prepare(&empty, frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)?;
    let mut hidden = Hidden::new(document, &apartment)?;
    let hwnd = hidden.hwnd.ok_or("Native child absent")?;
    let mut documents = Documents::new(hidden.document.take().ok_or("Document absent")?);
    publish(hwnd, &mut documents, true)?;
    let engine = engine::find(hwnd)?;
    let mut storage = None;
    let target = Dib::for_frame(&mut storage, &frame)?;
    for batch in 0..16 {
        let first = batch * 128 + 1;
        let count = [1, 16, 128, 3, 128][batch as usize % 5];
        let text: Vec<_> = (first..first + count)
            .map(|id| {
                if id % 13 == 0 {
                    String::new()
                } else {
                    format!(
                        "Import {id}: {}",
                        "dependency ".repeat(if id % 17 == 0 { 300 } else { id as usize % 9 })
                    )
                }
            })
            .collect();
        let rows: Vec<_> = (first..first + count).zip(text.iter().map(String::as_str)).collect();
        let mut source = source(batch + 2, &rows)?;
        for (index, record) in source.records.iter_mut().enumerate() {
            let text = record.text().to_owned();
            let runs = if text.is_empty() {
                Vec::new()
            } else {
                let split = (text.len() / 2) as u32;
                vec![
                    StyleRun {
                        start: 0,
                        end: split,
                        style: TextStyle {
                            foreground: if batch % 2 == 0 {
                                AnsiColor::Default
                            } else {
                                AnsiColor::Indexed((index % 16) as u8)
                            },
                            ..Default::default()
                        },
                    },
                    StyleRun {
                        start: split,
                        end: text.len() as u32,
                        style: TextStyle {
                            foreground: AnsiColor::Indexed(2),
                            bold: index % 2 == 0,
                            ..Default::default()
                        },
                    },
                ]
            };
            *record = TranscriptRecord::new(record.id(), OutputStream::Stdout, text, runs, true)?;
        }
        let (prepared, _source_frame, _source_release) = index(&source)?;
        documents.stage(prepared)?;
        publish(hwnd, &mut documents, true)?;
        engine.prepare(
            &frame,
            Some(documents.shared().ok_or("Shared document absent")?),
            CROP,
            true,
            1.0,
        )?;
        engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
        assert!(engine.at_bottom()?, "Mixed output stopped following the tail in batch {batch}");
        let control = messages::Messages(hwnd);
        for lines in [-3, -11, -31, -200] {
            control.scroll_lines(lines)?;
            engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
        }
        control.scroll_bottom()?;
        engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
    }
    target.close()?;
    hidden.close()?;
    drop((engine, documents, hidden, frame));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    // SAFETY: Every original hidden window and native console owner has retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}

/// Real RichEdit publication, alpha drawing and exposure exercise the production native path.
#[test]
fn native_console_burst_publication_and_cached_exposure() -> TestResult {
    let mut apartment = UiApartment::enter()?;
    let (frame, released) = frame()?;
    let empty = source(1, &[])?;
    let document =
        DocumentIndex::prepare(&empty, frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)?;
    let mut hidden = Hidden::new(document, &apartment)?;
    let hwnd = hidden.hwnd.ok_or("Native child absent")?;
    let mut documents = Documents::new(hidden.document.take().ok_or("Document absent")?);
    publish(hwnd, &mut documents, true)?;
    let engine = engine::find(hwnd)?;
    let mut storage = None;
    let target = Dib::for_frame(&mut storage, &frame)?;
    let mut publication = Vec::new();
    let mut painting = Vec::new();
    let mut exposure = Vec::new();
    for batch in 0..32 {
        let first = batch * 16 + 1;
        let text: Vec<_> = (first..first + 128)
            .map(|id| {
                format!(
                    "Dependency {id}: {}",
                    "Downloading 日本語 😀 ".repeat((id % 7 + 1) as usize)
                )
            })
            .collect();
        let rows: Vec<_> = (first..first + 128).zip(text.iter().map(String::as_str)).collect();
        let source = source(batch + 2, &rows)?;
        let (prepared, _source_frame, _source_release) = index(&source)?;
        documents.stage(prepared)?;
        let start = Instant::now();
        publish(hwnd, &mut documents, true)?;
        publication.push(start.elapsed().as_secs_f64() * 1000.0);
        engine.prepare(
            &frame,
            Some(documents.shared().ok_or("Shared document absent")?),
            CROP,
            true,
            1.0,
        )?;
        let start = Instant::now();
        engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
        painting.push(start.elapsed().as_secs_f64() * 1000.0);
        assert!(engine.at_bottom()?, "burst publication stopped following the tail");
        let draws = engine.qualified_glyph_draws();
        let start = Instant::now();
        engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
        exposure.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(engine.qualified_glyph_draws(), draws);
    }
    for (name, mut values) in
        [("publication", publication), ("paint", painting), ("cached_expose", exposure)]
    {
        values.sort_by(f64::total_cmp);
        println!(
            "native_console_{name}_ms median={:.3} p95={:.3} max={:.3}",
            values[16], values[30], values[31]
        );
    }
    target.close()?;
    hidden.close()?;
    drop((engine, documents, hidden, frame));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    // SAFETY: Every original hidden window and native console owner has retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}
