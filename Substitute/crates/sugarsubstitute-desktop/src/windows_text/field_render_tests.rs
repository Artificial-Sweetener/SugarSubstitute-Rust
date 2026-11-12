// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify field pixels and exact viewport admission against the real native document.
use super::{Engine, TestResult};
use crate::native_storage::Dib;
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameShape, FrameValidator, NATIVE_CONSOLE_ACCESSIBILITY_BYTES},
};
use windows::Win32::{
    Foundation::{LPARAM, RECT, WPARAM},
    UI::Controls::{EM_GETSEL, EM_SETSEL},
};
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
/// The fixture deliberately funds two small field copies, never two whole-window copies.
pub(super) fn qualify(engine: &Engine) -> TestResult {
    let shape = FrameShape::new(300, 100, 1.0)?;
    let field_bytes = 250 * 32 * 4;
    let lease = MemoryLease::new(
        2 * shape.byte_len() as u64 + 2 * field_bytes + NATIVE_CONSOLE_ACCESSIBILITY_BYTES,
        Box::new(Release),
    );
    let mut pixels = vec![0; shape.byte_len()];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[24, 25, 26, 255]);
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
    while !validator.validate_next(NonZeroUsize::MIN.saturating_add(4095))? {}
    let frame = validator.finish()?;
    let mut storage = None;
    let output = Dib::for_frame(&mut storage, &frame)?;
    engine.prepare(&frame, None, RECT { left: 0, top: 40, right: 250, bottom: 72 }, true, 1.0)?;
    assert!(frame.claim_storage(1).is_err(), "field rendering must claim exactly its viewport");
    engine.message(EM_SETSEL, WPARAM(1), LPARAM(5))?;
    let selection = engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?;
    let revision = engine.state.revision.get();
    engine.qualify_pixels(output.dc(), true, output.pixels_mut()?)?;
    assert!(
        output.pixels_mut()?.chunks_exact(4).any(|pixel| pixel[3] > 0),
        "native field drew no glyphs or selection"
    );
    assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selection);
    assert_eq!(engine.state.revision.get(), revision);
    let draws = engine.qualified_glyph_draws();
    engine.qualify_pixels(output.dc(), false, output.pixels_mut()?)?;
    assert_eq!(engine.qualified_glyph_draws(), draws, "exposure must reuse native glyphs");
    engine.message(EM_SETSEL, WPARAM(0), LPARAM(0))?;
    engine.qualify_pixels(output.dc(), false, output.pixels_mut()?)?;
    assert_eq!(engine.qualified_glyph_draws(), draws + 1, "selection change must redraw once");
    // The same real document changes mode without replacing text, selection or revision.
    let selection = engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?;
    let value = engine.field_value()?;
    for dark in [true, false, true] {
        engine.prepare(
            &frame,
            None,
            RECT { left: 0, top: 40, right: 250, bottom: 72 },
            dark,
            1.0,
        )?;
        engine.qualify_pixels(output.dc(), true, output.pixels_mut()?)?;
        let pixels = output.pixels_mut()?;
        assert!(
            (6..28).any(|y| (20..180).any(|x| {
                let pixel = &pixels[((y * 300 + x) * 4)..][..4];
                pixel[3] == 255
                    && if dark {
                        pixel[..3].iter().all(|channel| *channel > 200)
                    } else {
                        pixel[..3].iter().all(|channel| *channel < 8)
                    }
            })),
            "Native field lacks resolved foreground in dark={dark}"
        );
        assert_eq!(engine.field_value()?.text, value.text);
        assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selection);
        assert_eq!(engine.state.revision.get(), revision);
    }
    output.close()?;
    Ok(())
}
