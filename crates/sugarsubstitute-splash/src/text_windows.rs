// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own native Windows font shaping and bounded grayscale status coverage.
//!
//! DirectWrite objects and callback buffers remain within this owner. Draw is
//! synchronous; copied runs retain their font faces. No HWND or COM apartment
//! initialization is required by the shared DirectWrite factory. The owner is
//! used on one presentation thread and exposes only ordinary owned pixel data.

use std::{cell::RefCell, io, mem::ManuallyDrop};
use tiny_skia::Pixmap;
use windows::{
    Win32::Graphics::DirectWrite::*,
    core::{ComObject, Interface, PCWSTR, w},
};

use crate::Theme;

#[path = "text_windows_runs.rs"]
mod runs;
use runs::{Collector, GlyphRun};

/// Retain native font discovery after the first splash submission.
pub(crate) struct StatusText {
    factory: IDWriteFactory2,
    format: IDWriteTextFormat,
}

impl StatusText {
    /// Prepare the installed Segoe UI family with locale-aware system fallback.
    pub(crate) fn new(locale: &str) -> Result<Self, io::Error> {
        if locale.len() > 64 || locale.contains('\0') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid status font locale"));
        }
        let locale: Vec<u16> = locale.encode_utf16().chain(Some(0)).collect();
        // SAFETY: Factory creation owns its returned COM reference. UTF-16 pointers
        // remain terminated and alive through CreateTextFormat, which copies them.
        let (factory, format) = unsafe {
            let factory: IDWriteFactory2 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                14.0,
                PCWSTR(locale.as_ptr()),
            )?;
            let builder = factory.CreateFontFallbackBuilder()?;
            // Preserve the production label's explicit family priority, then
            // delegate uncovered scripts to the installed Windows fallback owner.
            builder.AddMapping(
                &[DWRITE_UNICODE_RANGE { first: 0, last: 0x10ffff }],
                &[
                    w!("Microsoft YaHei").as_ptr(),
                    w!("PingFang SC").as_ptr(),
                    // Qt's Windows fallback family search selects Gulim for Hangul.
                    w!("Gulim").as_ptr(),
                ],
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                1.0,
            )?;
            builder.AddMappings(&factory.GetSystemFontFallback()?)?;
            format.cast::<IDWriteTextFormat1>()?.SetFontFallback(&builder.CreateFontFallback()?)?;
            (factory, format)
        };
        Ok(Self { factory, format })
    }

    /// Paint bottom-aligned native glyphs in device pixels after bounded shaping.
    ///
    /// Errors retain native shaping/raster causes. UTF-16 input is bounded to
    /// 4096 units, positive device scales to four and raster textures to four MiB.
    pub(crate) fn paint(
        &mut self,
        target: &mut Pixmap,
        message: &str,
        theme: Theme,
        scale: f32,
    ) -> Result<(), io::Error> {
        if !scale.is_finite() || scale <= 0.0 || scale > 4.0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid status device scale"));
        }
        let runs = self.shape(message)?;
        let last_baseline = runs.iter().map(|run| run.baseline.1).reduce(f32::max).unwrap_or(15.0);
        // QWidget clips its QLabel paint to the widget, including the eight-pixel
        // bottom contents margin: QRect(24, 486, 510, 44) in logical pixels.
        let mut surface = LabelSurface {
            pixels: target,
            bounds: [
                (24.0 * scale).round() as i32,
                (486.0 * scale).round() as i32,
                (534.0 * scale).round() as i32,
                (530.0 * scale).round() as i32,
            ],
        };
        for run in &runs {
            self.paint_run(&mut surface, run, theme, scale, 518.0 - last_baseline)?;
        }
        Ok(())
    }

    /// Copy bounded native shaping into independent font-face and glyph ownership.
    fn shape(&self, message: &str) -> Result<Vec<GlyphRun>, io::Error> {
        let text: Vec<u16> = message.encode_utf16().take(4097).collect();
        if text.len() > 4096 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Status exceeds UTF-16 bound"));
        }
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let collector = ComObject::new(Collector { runs: RefCell::new(Vec::new()) });
        let renderer: IDWriteTextRenderer = collector.to_interface();
        // SAFETY: Input slice and owned format remain valid through layout creation.
        // Draw is synchronous and retains the renderer for each callback invocation.
        unsafe {
            let layout = self.factory.CreateGdiCompatibleTextLayout(
                &text,
                &self.format,
                510.0,
                36.0,
                1.0,
                None,
                false,
            )?;
            layout.Draw(None, &renderer, 0.0, 0.0)?;
        }
        Ok(collector.get().runs.replace(Vec::new()))
    }

    /// Rasterize one retained native run and composite its bounded coverage.
    ///
    /// Qt's glyph cache retains the logical font size with a device transform.
    /// Its default full hinting floors each glyph's device X and rounds baseline Y.
    fn paint_run(
        &self,
        target: &mut LabelSurface<'_>,
        run: &GlyphRun,
        theme: Theme,
        scale: f32,
        origin_y: f32,
    ) -> Result<(), io::Error> {
        let mut pen = run.baseline.0;
        for (index, glyph) in run.indices.iter().enumerate() {
            let offset = DWRITE_GLYPH_OFFSET {
                advanceOffset: run.offsets[index].advanceOffset,
                ascenderOffset: run.offsets[index].ascenderOffset,
            };
            let raw = DWRITE_GLYPH_RUN {
                fontFace: ManuallyDrop::new(Some(run.face.clone())),
                fontEmSize: run.size,
                glyphCount: 1,
                glyphIndices: glyph,
                glyphAdvances: &0.0,
                glyphOffsets: &offset,
                isSideways: false.into(),
                bidiLevel: run.level,
            };
            self.paint_glyph(
                target,
                raw,
                theme,
                ((24.0 + pen) * scale).floor(),
                ((origin_y + run.baseline.1) * scale).round(),
                scale,
            )?;
            pen += if run.level.is_multiple_of(2) {
                run.advances[index]
            } else {
                -run.advances[index]
            };
        }
        Ok(())
    }

    /// Keep default full-hinting glyph coverage on the native device pixel grid.
    fn paint_glyph(
        &self,
        target: &mut LabelSurface<'_>,
        mut raw: DWRITE_GLYPH_RUN,
        theme: Theme,
        x: f32,
        y: f32,
        scale: f32,
    ) -> Result<(), io::Error> {
        let mode = if raw.fontEmSize > 16.0 {
            DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC
        } else {
            DWRITE_RENDERING_MODE_GDI_CLASSIC
        };
        let measuring = if raw.fontEmSize > 16.0 {
            DWRITE_MEASURING_MODE_NATURAL
        } else {
            DWRITE_MEASURING_MODE_GDI_CLASSIC
        };
        // SAFETY: All run arrays have the same validated length and remain alive
        // through the synchronous analysis. The COM face outlives analysis creation.
        let analysis = unsafe {
            let transform = DWRITE_MATRIX { m11: scale, m22: scale, ..Default::default() };
            self.factory.CreateGlyphRunAnalysis(
                &raw,
                Some(&transform),
                mode,
                measuring,
                DWRITE_GRID_FIT_MODE_DEFAULT,
                DWRITE_TEXT_ANTIALIAS_MODE_CLEARTYPE,
                x / scale,
                y / scale,
            )
        };
        // The ABI field has no automatic destructor; release our owned clone.
        // SAFETY: This field owns exactly one clone and is no longer used by analysis.
        unsafe { ManuallyDrop::drop(&mut raw.fontFace) };
        let analysis = analysis?;
        // SAFETY: Analysis returns an owned rectangle and writes into its exact-size slice.
        let bounds = unsafe { analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)? };
        let width = bounds
            .right
            .checked_sub(bounds.left)
            .ok_or_else(|| io::Error::other("Invalid native glyph width"))?;
        let height = bounds
            .bottom
            .checked_sub(bounds.top)
            .ok_or_else(|| io::Error::other("Invalid native glyph height"))?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let length = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(3))
            .filter(|length| *length <= 4 * 1024 * 1024)
            .ok_or_else(|| io::Error::other("Status raster exceeds memory bound"))?;
        let mut coverage = vec![0; length];
        // SAFETY: The rectangle and allocated slice have exactly width*height*3 bytes.
        unsafe {
            analysis.CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bounds, &mut coverage)?
        };
        for y in 0..height {
            for x in 0..width {
                let offset = (y as usize * width as usize + x as usize) * 3;
                let channels = &coverage[offset..offset + 3];
                // Qt's transparent ARGB surface reduces ClearType coverage via
                // qdrawhelper.cpp's qRgbAvg: red/green/blue weights 5/6/5. This
                // path applies no display gamma or font-smoothing contrast.
                let gray = (u32::from(channels[0]) * 5
                    + u32::from(channels[1]) * 6
                    + u32::from(channels[2]) * 5)
                    / 16;
                target.composite(bounds.left + x, bounds.top + y, gray as u8, theme);
            }
        }
        Ok(())
    }
}

/// Borrow the destination with the QLabel's half-open device-pixel clip rectangle.
struct LabelSurface<'a> {
    pixels: &'a mut Pixmap,
    bounds: [i32; 4],
}

impl LabelSurface<'_> {
    /// Clip font overhang before compositing while preserving premultiplication.
    fn composite(&mut self, x: i32, y: i32, alpha: u8, theme: Theme) {
        if x < self.bounds[0]
            || y < self.bounds[1]
            || x >= self.bounds[2]
            || y >= self.bounds[3]
            || x < 0
            || y < 0
            || x >= self.pixels.width() as i32
            || y >= self.pixels.height() as i32
            || alpha == 0
        {
            return;
        }
        let offset = (y as usize * self.pixels.width() as usize + x as usize) * 4;
        let pixel = &mut self.pixels.data_mut()[offset..offset + 4];
        let source = if theme == Theme::Dark { alpha } else { 0 };
        for channel in pixel.iter_mut().take(3) {
            *channel = source.saturating_add(multiply(*channel, 255 - alpha));
        }
        pixel[3] = alpha.saturating_add(multiply(pixel[3], 255 - alpha));
    }
}

/// Divide the byte product with nearest integer coverage rounding.
fn multiply(left: u8, right: u8) -> u8 {
    let product = u32::from(left) * u32::from(right) + 128;
    ((product + (product >> 8)) >> 8) as u8
}

#[cfg(test)]
#[path = "text_windows_tests.rs"]
mod tests;
