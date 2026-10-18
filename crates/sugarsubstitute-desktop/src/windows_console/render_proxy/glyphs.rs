// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expand native color-font layers without reshaping or inferring document color from RGB.
use super::*;
use windows::{
    Win32::{
        Foundation::{DWRITE_E_NOCOLOR, E_POINTER},
        Graphics::DirectWrite::*,
    },
    core::Result,
};
use windows_numerics::{Matrix3x2, Vector2};

/// Restore the enclosing callback's diagnostic facts after native reentrancy or early return.
pub(super) struct GlyphScope<'a> {
    shared: &'a Shared,
    previous: Option<GlyphFacts>,
}
impl Drop for GlyphScope<'_> {
    fn drop(&mut self) {
        self.shared.active_glyph.set(self.previous);
    }
}

impl Shared {
    /// Copy valid scalar run evidence before brush decoding, retaining no borrowed native pointer.
    pub(super) fn glyph_scope(
        &self,
        run: *const DWRITE_GLYPH_RUN,
        measuring: DWRITE_MEASURING_MODE,
    ) -> GlyphScope<'_> {
        let facts = if run.is_null() {
            None
        } else {
            // SAFETY: RichEdit supplies a live native structure throughout this synchronous callback.
            let run = unsafe { &*run };
            let first = if run.glyphCount == 0 || run.glyphIndices.is_null() {
                None
            } else {
                // SAFETY: A nonempty native glyph run owns glyphCount readable glyph indices.
                Some(unsafe { *run.glyphIndices })
            };
            Some(GlyphFacts {
                count: run.glyphCount,
                em_bits: run.fontEmSize.to_bits(),
                measuring: measuring.0,
                first,
            })
        };
        let previous = self.active_glyph.replace(facts);
        GlyphScope { shared: self, previous }
    }
    /// Preserve the native run's positions and measuring mode while translating palette layers.
    /// The supplied brush already carries the semantic foreground RGBA for this run.
    pub(super) fn glyphs(
        &self,
        origin: Vector2,
        run: *const DWRITE_GLYPH_RUN,
        foreground: &ID2D1Brush,
        measuring: DWRITE_MEASURING_MODE,
    ) -> Result<()> {
        if run.is_null() {
            return Err(self.fail_at("DrawGlyphRun.run", E_POINTER, Detail::Native));
        }
        // SAFETY: RichEdit retains the complete native run for this synchronous render callback.
        let source = unsafe { &*run };
        if source.glyphCount != 0 && source.glyphIndices.is_null() {
            return Err(self.fail_at("DrawGlyphRun.indices", E_POINTER, Detail::Native));
        }
        reject_unsupported_color(self, source, false)?;
        let mut transform = Matrix3x2::default();
        // SAFETY: The scalar output remains writable until the owned target returns.
        unsafe { self.target.GetTransform(&mut transform) };
        let matrix = DWRITE_MATRIX {
            m11: transform.M11,
            m12: transform.M12,
            m21: transform.M21,
            m22: transform.M22,
            dx: transform.M31,
            dy: transform.M32,
        };
        // SAFETY: The native source run and transform remain live through synchronous translation.
        let translated = unsafe {
            self.factory.TranslateColorGlyphRun(
                origin.X,
                origin.Y,
                run,
                None,
                measuring,
                Some(&matrix),
                0,
            )
        };
        let layers = match translated {
            Ok(layers) => layers,
            Err(cause) if cause.code() == DWRITE_E_NOCOLOR => {
                reject_unsupported_color(self, source, true)?;
                // SAFETY: The original run and mapped foreground remain owned during this call.
                unsafe { self.target.DrawGlyphRun(origin, run, foreground, measuring) };
                return Ok(());
            }
            Err(cause) => {
                return Err(self.fail_at(
                    "DrawGlyphRun.TranslateColorGlyphRun",
                    cause.code(),
                    Detail::Native,
                ));
            }
        };
        let mut produced = false;
        // SAFETY: The enumerator remains owned and its current layer is borrowed until MoveNext.
        loop {
            // SAFETY: The owned enumerator remains live throughout this synchronous advance.
            let next = self.record("DrawGlyphRun.MoveNext", unsafe { layers.MoveNext() })?;
            if !next.as_bool() {
                break;
            }
            produced = true;
            // SAFETY: A successful MoveNext exposes a borrowed current run until the next advance.
            let pointer =
                self.record("DrawGlyphRun.GetCurrentRun", unsafe { layers.GetCurrentRun() })?;
            if pointer.is_null() {
                return Err(self.fail_at("DrawGlyphRun.current_layer", E_POINTER, Detail::Native));
            }
            // SAFETY: The checked native output remains live until this iteration completes.
            let layer = unsafe { &*pointer };
            let baseline = Vector2 { X: layer.baselineOriginX, Y: layer.baselineOriginY };
            if u32::from(layer.paletteIndex) == DWRITE_NO_PALETTE_INDEX {
                // SAFETY: The layer's original native geometry and mapped semantic brush are live.
                unsafe {
                    self.target.DrawGlyphRun(baseline, &layer.glyphRun, foreground, measuring)
                };
            } else {
                let color = layer.runColor;
                if [color.r, color.g, color.b, color.a]
                    .iter()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                {
                    return Err(self.fail_at(
                        "DrawGlyphRun.palette",
                        E_INVALIDARG,
                        Detail::Palette([
                            color.r.to_bits(),
                            color.g.to_bits(),
                            color.b.to_bits(),
                            color.a.to_bits(),
                        ]),
                    ));
                }
                let rgba = D2D1_COLOR_F { r: color.r, g: color.g, b: color.b, a: color.a };
                // SAFETY: Palette color belongs to the trusted native font, independent of text
                // semantics. The local value and resulting brush stay live through this layer.
                let brush = self.record("DrawGlyphRun.CreatePaletteBrush", unsafe {
                    self.target.CreateSolidColorBrush(&rgba, None)
                })?;
                // SAFETY: Native color-layer geometry remains borrowed until the next MoveNext.
                unsafe { self.target.DrawGlyphRun(baseline, &layer.glyphRun, &brush, measuring) };
            }
        }
        if produced {
            Ok(())
        } else {
            Err(self.fail_at("DrawGlyphRun.color_layers", E_INVALIDARG, Detail::EmptyColorLayers))
        }
    }
}

/// A no-color result may fall back only for genuine outline glyphs, never unsupported images.
fn reject_unsupported_color(
    shared: &Shared,
    run: &DWRITE_GLYPH_RUN,
    no_translation: bool,
) -> Result<()> {
    let face = run
        .fontFace
        .as_ref()
        .ok_or_else(|| shared.fail_at("DrawGlyphRun.fontFace", E_POINTER, Detail::Native))?;
    let face: IDWriteFontFace4 = shared.record("DrawGlyphRun.fontFace4", face.cast())?;
    let unsupported = DWRITE_GLYPH_IMAGE_FORMATS_SVG
        | DWRITE_GLYPH_IMAGE_FORMATS_PNG
        | DWRITE_GLYPH_IMAGE_FORMATS_JPEG
        | DWRITE_GLYPH_IMAGE_FORMATS_TIFF
        | DWRITE_GLYPH_IMAGE_FORMATS_PREMULTIPLIED_B8G8R8A8
        | DWRITE_GLYPH_IMAGE_FORMATS_COLR_PAINT_TREE;
    // SAFETY: Querying available formats does not outlive the owned native font face.
    if unsafe { face.GetGlyphImageFormats2() }.0 & unsupported.0 == 0 {
        return Ok(());
    }
    for index in 0..run.glyphCount {
        // SAFETY: The caller validated a live native run and its glyphCount input array.
        let glyph = unsafe { *run.glyphIndices.add(index as usize) };
        // SAFETY: The owned font face synchronously reports formats for this scalar glyph ID.
        let formats = shared.record("DrawGlyphRun.GetGlyphImageFormats", unsafe {
            face.GetGlyphImageFormats(glyph, 0, u32::MAX)
        })?;
        if formats.0 & unsupported.0 != 0
            && (no_translation || formats.0 & DWRITE_GLYPH_IMAGE_FORMATS_COLR.0 == 0)
        {
            return Err(shared.fail_at(
                "DrawGlyphRun.image_formats",
                E_NOTIMPL,
                Detail::GlyphFormat { glyph, formats: formats.0 },
            ));
        }
    }
    Ok(())
}
