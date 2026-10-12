// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Initialize and composite one bounded DirectWrite texture across cooperative steps.

use super::{GlyphRun, LabelSurface};
use crate::{
    Theme,
    text::memory::{self, Allocation, PaintSpec, TEXTURE_BYTES},
};
use std::{io, mem::ManuallyDrop, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::MemoryClaim;
use tiny_skia::Pixmap;
use windows::Win32::{Foundation::RECT, Graphics::DirectWrite::*};

/// Own shaping, native analysis and admitted coverage without borrowing a font owner.
///
/// Each step performs one bounded native operation or visits at most 65,536 pixels
/// while initializing/compositing. Native analysis and texture calls are indivisible
/// and limited to a single glyph and a four-MiB texture; their return is observed
/// before the execution owner can claim cancellation has stopped this work.
pub(crate) struct GlyphPaint {
    factory: IDWriteFactory2,
    runs: Vec<GlyphRun>,
    texture: Option<Texture>,
    run: usize,
    glyph: usize,
    pen: f32,
    baseline: f32,
    origin_x: f32,
    background: Option<super::console::ConsoleBackground>,
    theme: Theme,
    scale: f32,
    width: u32,
    height: u32,
    pub(super) clip: [i32; 4],
    allocation: Allocation,
    // All owned shaping arrays precede their guard in destruction order.
    _metadata: Option<MemoryClaim>,
}

struct Texture {
    analysis: IDWriteGlyphRunAnalysis,
    coverage: Vec<u8>,
    bounds: RECT,
    width: usize,
    length: usize,
    position: usize,
    ready: bool,
    // Native analysis and Rust coverage are dropped before capacity becomes reusable.
    _claim: Option<MemoryClaim>,
}

impl GlyphPaint {
    pub(super) fn new(
        factory: IDWriteFactory2,
        runs: Vec<GlyphRun>,
        metadata: Option<MemoryClaim>,
        allocation: Allocation,
        spec: PaintSpec,
    ) -> Self {
        let PaintSpec { theme, scale, width, height } = spec;
        let baseline =
            518.0 - runs.iter().map(|run| run.baseline.1).reduce(f32::max).unwrap_or(15.0);
        let pen = runs.first().map_or(0.0, |run| run.baseline.0);
        Self {
            factory,
            runs,
            texture: None,
            run: 0,
            glyph: 0,
            pen,
            baseline,
            origin_x: 24.0,
            background: None,
            theme,
            scale,
            width,
            height,
            clip: [
                (24.0 * scale).round() as i32,
                (486.0 * scale).round() as i32,
                (534.0 * scale).round() as i32,
                (530.0 * scale).round() as i32,
            ],
            allocation,
            _metadata: metadata,
        }
    }

    /// Retain actual console layout geometry until every bounded background query completes.
    pub(super) fn bind_console(
        &mut self,
        spec: crate::text::console::ConsolePaintSpec,
        background: super::console::ConsoleBackground,
    ) {
        self.origin_x = 30.0;
        self.baseline = spec.top();
        self.clip = spec.clip();
        self.clip[1] = self.clip[1].max((spec.top() * self.scale).round() as i32);
        self.clip[3] = self.clip[3].min(((spec.top() + 16.0) * self.scale).round() as i32);
        self.background = Some(background);
    }

    /// Advance bounded authored work into the original device-sized destination.
    ///
    /// Returns true only after every glyph is composited. A different target shape
    /// is rejected before mutation. Scratch allocations remain claimed through Drop.
    pub(crate) fn step(&mut self, target: &mut Pixmap, max: NonZeroUsize) -> io::Result<bool> {
        if target.width() != self.width || target.height() != self.height {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Status target shape changed"));
        }
        if let Some(background) = &mut self.background {
            if background.step(target, max)? {
                self.background = None;
            }
            return Ok(false);
        }
        if self.run == self.runs.len() {
            return Ok(true);
        }
        if self.texture.is_none() {
            self.texture = self.analyze()?;
            if self.texture.is_none() {
                self.advance();
            }
            return Ok(self.run == self.runs.len());
        }
        let texture =
            self.texture.as_mut().ok_or_else(|| io::Error::other("Missing glyph texture"))?;
        let max = memory::budget(max);
        if texture.coverage.len() < texture.length {
            // One coverage pixel has three ClearType bytes; the pixel budget also
            // bounds this initialization, without an unbounded vec![0; length].
            let end = texture.coverage.len().saturating_add(max * 3).min(texture.length);
            texture.coverage.resize(end, 0);
            return Ok(false);
        }
        if !texture.ready {
            // SAFETY: Analysis owns the validated rectangle; coverage is initialized
            // and has exactly width*height*3 bytes, bounded to four MiB.
            unsafe {
                texture.analysis.CreateAlphaTexture(
                    DWRITE_TEXTURE_CLEARTYPE_3x1,
                    &texture.bounds,
                    &mut texture.coverage,
                )?;
            }
            texture.ready = true;
            return Ok(false);
        }
        let end = texture.position.saturating_add(max).min(texture.length / 3);
        let mut surface = LabelSurface { pixels: target, bounds: self.clip };
        for index in texture.position..end {
            let channels = &texture.coverage[index * 3..index * 3 + 3];
            // Qt qRgbAvg reduces transparent ClearType via 5/6/5, without gamma.
            let gray = (u32::from(channels[0]) * 5
                + u32::from(channels[1]) * 6
                + u32::from(channels[2]) * 5)
                / 16;
            let x = texture.bounds.left + (index % texture.width) as i32;
            let y = texture.bounds.top + (index / texture.width) as i32;
            let color = self.runs[self.run].color.unwrap_or(if self.theme == Theme::Dark {
                [255; 3]
            } else {
                [0; 3]
            });
            surface.composite(x, y, gray as u8, color);
        }
        texture.position = end;
        if end == texture.length / 3 {
            self.texture = None;
            self.advance();
        }
        Ok(self.run == self.runs.len())
    }

    fn advance(&mut self) {
        let run = &self.runs[self.run];
        self.pen += if run.level.is_multiple_of(2) {
            run.advances[self.glyph]
        } else {
            -run.advances[self.glyph]
        };
        self.glyph += 1;
        if self.glyph == run.indices.len() {
            self.run += 1;
            self.glyph = 0;
            self.pen = self.runs.get(self.run).map_or(0.0, |run| run.baseline.0);
        }
    }

    /// Analyze exactly one glyph, preserving the reference device hinting positions.
    fn analyze(&self) -> io::Result<Option<Texture>> {
        let run = &self.runs[self.run];
        let offset = &run.offsets[self.glyph];
        let mut raw = DWRITE_GLYPH_RUN {
            fontFace: ManuallyDrop::new(Some(run.face.clone())),
            fontEmSize: run.size,
            glyphCount: 1,
            glyphIndices: &run.indices[self.glyph],
            glyphAdvances: &0.0,
            glyphOffsets: offset,
            isSideways: false.into(),
            bidiLevel: run.level,
        };
        let x = ((self.origin_x + self.pen) * self.scale).floor();
        let y = ((self.baseline + run.baseline.1) * self.scale).round();
        let mode = if run.size > 16.0 {
            DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC
        } else {
            DWRITE_RENDERING_MODE_GDI_CLASSIC
        };
        let measuring = if run.size > 16.0 {
            DWRITE_MEASURING_MODE_NATURAL
        } else {
            DWRITE_MEASURING_MODE_GDI_CLASSIC
        };
        let analysis = if x.is_finite()
            && y.is_finite()
            && (x / self.scale).is_finite()
            && (y / self.scale).is_finite()
        {
            // SAFETY: This one-glyph run borrows retained arrays and a cloned face
            // through synchronous analysis. The finite positive transform is validated.
            unsafe {
                self.factory
                    .CreateGlyphRunAnalysis(
                        &raw,
                        Some(&DWRITE_MATRIX {
                            m11: self.scale,
                            m22: self.scale,
                            ..Default::default()
                        }),
                        mode,
                        measuring,
                        DWRITE_GRID_FIT_MODE_DEFAULT,
                        DWRITE_TEXT_ANTIALIAS_MODE_CLEARTYPE,
                        x / self.scale,
                        y / self.scale,
                    )
                    .map_err(io::Error::from)
            }
        } else {
            Err(io::Error::other("Invalid accumulated glyph position"))
        };
        // SAFETY: The ABI field owns exactly one clone and analysis no longer borrows it.
        unsafe { ManuallyDrop::drop(&mut raw.fontFace) };
        let analysis = analysis?;
        // SAFETY: The owned analysis returns a rectangle without retaining output pointers.
        let mut bounds = unsafe { analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)? };
        let width = bounds
            .right
            .checked_sub(bounds.left)
            .ok_or_else(|| io::Error::other("Invalid native glyph width"))?;
        let height = bounds
            .bottom
            .checked_sub(bounds.top)
            .ok_or_else(|| io::Error::other("Invalid native glyph height"))?;
        if width < 0 || height < 0 {
            return Err(io::Error::other("Invalid native glyph bounds"));
        }
        if width == 0 || height == 0 {
            return Ok(None);
        }
        // The DirectWrite CreateAlphaTexture contract permits a rectangle different
        // from GetAlphaTextureBounds. Crop before allocation to the QLabel/device
        // intersection, preserving coverage while avoiding invisible raster work.
        bounds.left = bounds.left.max(self.clip[0]).max(0);
        bounds.top = bounds.top.max(self.clip[1]).max(0);
        bounds.right = bounds.right.min(self.clip[2]).min(self.width as i32);
        bounds.bottom = bounds.bottom.min(self.clip[3]).min(self.height as i32);
        if bounds.right <= bounds.left || bounds.bottom <= bounds.top {
            return Ok(None);
        }
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let length = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(3))
            .filter(|n| *n <= TEXTURE_BYTES)
            .ok_or_else(|| io::Error::other("Status raster exceeds memory bound"))?;
        let claim = self.allocation.claim(TEXTURE_BYTES)?;
        let coverage = memory::reserve(length, TEXTURE_BYTES)?;
        Ok(Some(Texture {
            analysis,
            coverage,
            bounds,
            width: width as usize,
            length,
            position: 0,
            ready: false,
            _claim: claim,
        }))
    }
}

#[cfg(test)]
#[path = "text_windows_cursor_tests.rs"]
mod tests;
