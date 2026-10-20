// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual system RichEdit alpha and crop composition without displaying a window.
use super::*;
use crate::{
    console::update::Documents,
    native_storage::Dib,
    windows_controls::console::{engine, messages},
};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{
        ConsolePresentation, FrameIdentity, FrameShape, FrameValidator,
        NATIVE_CONSOLE_STORAGE_BYTES, PreparedFrame,
    },
    transcript::*,
};
use windows::{Win32::Foundation::RECT, core::Interface};

#[path = "performance_tests.rs"]
mod performance_tests;

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _bytes: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
pub(super) fn frame() -> Result<(PreparedFrame, Arc<AtomicUsize>), Box<dyn std::error::Error>> {
    frame_with_shape(FrameShape::new(350, 160, 1.0)?)
}
fn frame_with_shape(
    shape: FrameShape,
) -> Result<(PreparedFrame, Arc<AtomicUsize>), Box<dyn std::error::Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let lease = MemoryLease::new(
        ConsolePresentation::Native.storage_bytes(shape) + 2 * shape.byte_len() as u64,
        Box::new(Release(released.clone())),
    );
    let mut pixels = vec![0; shape.byte_len()];
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(&[(index % 47) as u8, ((index / 350) % 43) as u8, 32, 180]);
    }
    let mut validator = FrameValidator::new(
        shape,
        FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        },
        pixels,
        lease,
    )?;
    while !validator.validate_next(NonZeroUsize::new(4096).ok_or("validation chunk zero")?)? {}
    Ok((validator.finish()?, released))
}
fn document(
    frame: &PreparedFrame,
    text: &str,
) -> Result<DocumentIndex, Box<dyn std::error::Error>> {
    let mut source = source(1, &[(1, text)])?;
    if !text.is_empty() {
        let split = if text.starts_with("HH") { 2 } else { text.len() };
        let mut runs =
            vec![StyleRun { start: 0, end: 1.min(split) as u32, style: TextStyle::default() }];
        if split > 1 {
            runs.push(StyleRun {
                start: 1,
                end: split as u32,
                style: TextStyle {
                    foreground: AnsiColor::Rgb(230, 236, 241),
                    ..Default::default()
                },
            });
        }
        if split < text.len() {
            runs.push(StyleRun {
                start: split as u32,
                end: text.len() as u32,
                style: TextStyle::default(),
            });
        }
        source.records[0] = TranscriptRecord::new(
            RecordId::new(1)?,
            OutputStream::Stdout,
            text.to_owned(),
            runs,
            true,
        )?;
    }
    Ok(DocumentIndex::prepare(&source, frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?)?)
}
const CROP: RECT = RECT { left: 25, top: 20, right: 325, bottom: 140 };

#[test]
fn native_alpha_console_preserves_exact_crop_semantic_alpha_unicode_selection_and_retirement()
-> TestResult {
    let mut apartment = UiApartment::enter()?;
    for text in ["", "HHe\u{301} שלום 日本 😀 👩‍💻"] {
        let (frame, released) = frame()?;
        let prepared = document(&frame, text)?;
        let mut hidden = Hidden::new(prepared, &apartment)?;
        let hwnd = hidden.hwnd.ok_or("native child absent")?;
        let mut documents = Documents::new(hidden.document.take().ok_or("original index absent")?);
        publish(hwnd, &mut documents, true)?;
        let engine = engine::find(hwnd)?;
        let mut parent = None;
        let parent = Dib::for_frame(&mut parent, &frame)?;
        engine.prepare(
            &frame,
            documents.shared().ok_or("shared index absent")?,
            CROP,
            true,
            1.0,
        )?;
        assert!(
            frame.claim_storage(1).is_err(),
            "native copies and metadata must consume their exact admitted credits"
        );
        engine.qualify_pixels(parent.dc(), false, parent.pixels_mut()?).map_err(|cause| {
            DesktopError::new(
                if text.is_empty() {
                    "console_test_empty_composite"
                } else {
                    "console_test_unicode_composite"
                },
                cause,
            )
        })?;
        if text.is_empty() {
            let actual = parent.pixels_mut()?;
            for y in 0..120usize {
                for x in 0..300usize {
                    let source = (y + 20) * frame.stride() + (x + 25) * 4;
                    let target = y * frame.stride() + x * 4;
                    let rgba = &frame.pixels()[source..source + 4];
                    assert_eq!(
                        &actual[target..target + 4],
                        &[rgba[2], rgba[1], rgba[0], rgba[3]],
                        "empty native console changed the actual prepared wash/corner/mascot crop"
                    );
                }
            }
        } else {
            engine
                .qualify_pixels(parent.dc(), true, parent.pixels_mut()?)
                .map_err(|cause| DesktopError::new("console_test_unicode_alpha", cause))?;
            let first = rect(&engine, 0, 1)?;
            let second = rect(&engine, 1, 2)?;
            let bytes = parent.pixels_mut()?;
            assert_eq!(
                max_alpha(bytes, frame.stride(), first),
                234,
                "first={first:?}, second={second:?}, {}",
                layout_facts(&engine, 2)?
            );
            assert_eq!(max_alpha(bytes, frame.stride(), second), 255);
            assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] > 0));
            assert!(
                bytes.chunks_exact(4).any(|pixel| u16::from(pixel[2]) > u16::from(pixel[0]) + 20),
                "native first publication lost intrinsic emoji palette colors"
            );
            assert!(
                bytes
                    .chunks_exact(4)
                    .all(|pixel| pixel[..3].iter().all(|component| *component <= pixel[3]))
            );
            let control = messages::Messages(hwnd);
            control.select(0, 1)?;
            engine
                .qualify_pixels(parent.dc(), true, parent.pixels_mut()?)
                .map_err(|cause| DesktopError::new("console_test_selected_alpha", cause))?;
            assert_eq!(control.selection().cpMin, 0);
            assert_eq!(control.selection().cpMax, 1);
            assert!(parent.pixels_mut()?.chunks_exact(4).any(|pixel| pixel == [42, 13, 98, 107]));
            let selected = control.selected_format();
            assert_eq!(selected.Base.crTextColor.0, 230 | (236 << 8) | (241 << 16));
            let mut copied = vec![0u16; text.encode_utf16().count() + 1];
            assert_eq!(control.read(&mut copied)?, text.encode_utf16().count());
            assert_eq!(&copied[..copied.len() - 1], text.encode_utf16().collect::<Vec<_>>());

            let selected_pixels = fingerprint(parent.pixels_mut()?);
            engine.qualify_pixels(parent.dc(), true, parent.pixels_mut()?)?;
            assert_eq!(
                fingerprint(parent.pixels_mut()?),
                selected_pixels,
                "unchanged selected native document repaint changed alpha pixels"
            );
            let selected_rows = row_fingerprints(parent.pixels_mut()?, frame.stride());
            let selected_geometry = rect(&engine, 0, text.encode_utf16().count() as i32)?;
            let selected_layout = layout_facts(&engine, text.encode_utf16().count() as i32)?;
            // SAFETY: Force actual native reentry during a redraw-suspended partial replacement.
            unsafe {
                use windows::Win32::{
                    Foundation::{LPARAM, WPARAM},
                    UI::WindowsAndMessaging::{SendMessageW, WM_PAINT, WM_SETREDRAW},
                };
                SendMessageW(hwnd, WM_SETREDRAW, Some(WPARAM(0)), None);
                let mut offset = 0usize;
                let mut partial = EDITSTREAM {
                    dwCookie: (&raw mut offset) as usize,
                    dwError: 0,
                    pfnCallback: Some(partial_stream),
                };
                SendMessageW(
                    hwnd,
                    EM_STREAMIN,
                    Some(WPARAM((SF_TEXT | SF_UNICODE) as usize)),
                    Some(LPARAM((&raw mut partial) as isize)),
                );
                let error = partial.dwError;
                assert_eq!(error, 0);
                assert_eq!(offset, 2);
                SendMessageW(hwnd, WM_PAINT, None, None);
            }
            engine.qualified_alpha(parent.pixels_mut()?)?;
            assert_eq!(
                fingerprint(parent.pixels_mut()?),
                selected_pixels,
                "forced WM_PAINT exposed a partially replaced native document"
            );
            let mut port = NativeText { hwnd, dark: true };
            port.write(documents.current().ok_or("source index absent")?, None)?;
            port.restore(0, 1, 0, false)?;
            // SAFETY: Restore only this original child's redraw after the complete document returns.
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::WM_SETREDRAW,
                    Some(windows::Win32::Foundation::WPARAM(1)),
                    None,
                );
            }
            engine.qualify_pixels(parent.dc(), true, parent.pixels_mut()?)?;
            assert_eq!(
                fingerprint(parent.pixels_mut()?),
                selected_pixels,
                "republication changed alpha pixels: old_geometry={selected_geometry:?}, new_geometry={:?}, old_rows={selected_rows:x?}, new_rows={:x?}, old_font={:?}, new_font={:?}, old_layout={selected_layout}, new_layout={}",
                rect(&engine, 0, text.encode_utf16().count() as i32)?,
                row_fingerprints(parent.pixels_mut()?, frame.stride()),
                font_fact(selected),
                font_fact(control.selected_format()),
                layout_facts(&engine, text.encode_utf16().count() as i32)?,
            );
            engine.qualify_pixels(parent.dc(), false, parent.pixels_mut()?)?;
        }
        parent.close()?;
        hidden.close()?;
        assert!(engine.retired());
        drop((engine, documents, hidden, frame));
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }
    // SAFETY: Every original child, native provider, text service and system library retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}
fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

/// A long provisioning transcript may acquire its first native layout during paint.
#[test]
fn native_alpha_long_transcript_preserves_scroll_and_selection_on_first_paint() -> TestResult {
    let mut apartment = UiApartment::enter()?;
    let (frame, released) = frame()?;
    let lines: Vec<_> = (1..=128)
        .map(|id| (id, "Provisioning dependency 日本語 😀 with a long wrapped status message"))
        .collect();
    let source = source(128, &lines)?;
    let claim = frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?;
    let prepared = DocumentIndex::prepare(&source, claim)?;
    let mut hidden = Hidden::new(prepared, &apartment)?;
    let hwnd = hidden.hwnd.ok_or("Native child absent")?;
    let mut documents = Documents::new(hidden.document.take().ok_or("Document absent")?);
    publish(hwnd, &mut documents, true)?;
    let engine = engine::find(hwnd)?;
    let mut storage = None;
    let target = Dib::for_frame(&mut storage, &frame)?;
    engine.prepare(&frame, documents.shared().ok_or("Shared document absent")?, CROP, true, 1.0)?;
    let control = messages::Messages(hwnd);
    let selection = control.selection();
    let line = control.first_line()?;
    engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
    assert_eq!(control.selection().cpMin, selection.cpMin);
    assert_eq!(control.selection().cpMax, selection.cpMax);
    assert_eq!(control.first_line()?, line);
    let painted = fingerprint(target.pixels_mut()?);
    let draws = engine.qualified_glyph_draws();
    for _ in 0..4 {
        engine.prepare(
            &frame,
            documents.shared().ok_or("Shared document absent")?,
            CROP,
            true,
            1.0,
        )?;
        engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
        assert_eq!(fingerprint(target.pixels_mut()?), painted);
    }
    assert_eq!(engine.qualified_glyph_draws(), draws, "unchanged console rerasterized on expose");
    control.select(5, 14)?;
    control.scroll_lines(-3)?;
    let line = control.first_line()?;
    engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
    assert!(engine.qualified_glyph_draws() > draws, "input did not invalidate native glyphs");
    assert_eq!(control.first_line()?, line);
    assert_eq!(control.selection().cpMin, 5);
    assert_eq!(control.selection().cpMax, 14);
    for delta in [-50, -50, 25, 25] {
        control.scroll_lines(delta)?;
        let first = control.line_index(control.first_line()?)?;
        engine.qualify_pixels(target.dc(), false, target.pixels_mut()?)?;
        assert_eq!(control.line_index(control.first_line()?)?, first);
        assert_eq!((control.selection().cpMin, control.selection().cpMax), (5, 14));
    }
    target.close()?;
    hidden.close()?;
    drop((engine, documents, hidden, frame));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    // SAFETY: The original hidden window and all native console owners have retired.
    unsafe { apartment.release_after_teardown() }?;
    Ok(())
}
fn row_fingerprints(bytes: &[u8], stride: usize) -> [u64; 160] {
    std::array::from_fn(|row| fingerprint(&bytes[row * stride..(row + 1) * stride]))
}
fn font_fact(format: CHARFORMAT2W) -> (i32, i32, u32, u32, u32, [u16; 32]) {
    (
        format.Base.yHeight,
        format.Base.yOffset,
        format.Base.dwEffects.0,
        format.Base.crTextColor.0,
        format.crBackColor.0,
        format.Base.szFaceName,
    )
}
/// # Safety
/// The synchronous stream retains its stack cursor and the native output buffers for this call.
unsafe extern "system" fn partial_stream(
    cookie: usize,
    buffer: *mut u8,
    count: i32,
    written: *mut i32,
) -> u32 {
    if cookie == 0 || written.is_null() || count < 0 || (count > 0 && buffer.is_null()) {
        return 87;
    }
    // SAFETY: The only callsite supplies this exact live stack cursor and documented outputs.
    unsafe {
        let offset = &mut *(cookie as *mut usize);
        let source = [b'W', 0];
        if *offset > source.len() {
            return 87;
        }
        let available = (source.len() - *offset).min(count as usize);
        if available > 0 {
            std::ptr::copy_nonoverlapping(source.as_ptr().add(*offset), buffer, available);
        }
        *offset += available;
        written.write(available as i32);
    }
    0
}
fn layout_facts(engine: &engine::Engine, units: i32) -> Result<String, DesktopError> {
    let document: ITextDocument = engine
        .services()?
        .cast()
        .map_err(|cause| DesktopError::new("console_test_layout_document", cause))?;
    // SAFETY: Passive font/paragraph facts belong to this retained native test document.
    let observed: windows::core::Result<_> = unsafe {
        (|| {
            let mut fonts = Vec::new();
            for unit in 0..units {
                let font = document.Range(unit, unit + 1)?.GetFont()?;
                fonts.push((font.GetName()?.to_string(), font.GetSize()?, font.GetLanguageID()?));
            }
            let paragraph = document.Range(0, units)?.GetPara()?;
            Ok((
                fonts,
                (
                    paragraph.GetLineSpacing()?,
                    paragraph.GetLineSpacingRule()?,
                    paragraph.GetSpaceBefore()?,
                    paragraph.GetSpaceAfter()?,
                ),
            ))
        })()
    };
    let (fonts, paragraph) =
        observed.map_err(|cause| DesktopError::new("console_test_layout_facts", cause))?;
    Ok(format!("fonts={fonts:?}, paragraph={paragraph:?}"))
}
fn rect(engine: &engine::Engine, start: i32, end: i32) -> Result<RECT, DesktopError> {
    let document: ITextDocument = engine
        .services()?
        .cast()
        .map_err(|cause| DesktopError::new("console_test_geometry", cause))?;
    // SAFETY: Native TOM owns the exact glyph geometry; this test only observes its known range.
    let range: ITextRange2 = unsafe { document.Range(start, end) }
        .and_then(|range: ITextRange| range.cast::<ITextRange2>())
        .map_err(|cause| DesktopError::new("console_test_geometry", cause))?;
    let mut result = RECT::default();
    let mut hit = 0;
    // SAFETY: All writable scalar outputs belong to this fixture and native range remains retained.
    unsafe {
        range.GetRect(
            tomClientCoord.0 | tomAllowOffClient.0,
            &mut result.left,
            &mut result.top,
            &mut result.right,
            &mut result.bottom,
            &mut hit,
        )
    }
    .map_err(|cause| DesktopError::new("console_test_geometry", cause))?;
    Ok(result)
}
fn max_alpha(bytes: &[u8], stride: usize, rect: RECT) -> u8 {
    let mut alpha = 0;
    for y in rect.top.max(0)..rect.bottom.max(0) {
        for x in rect.left.max(0)..rect.right.max(0) {
            if let Some(pixel) = bytes.get(y as usize * stride + x as usize * 4 + 3) {
                alpha = alpha.max(*pixel);
            }
        }
    }
    alpha
}
