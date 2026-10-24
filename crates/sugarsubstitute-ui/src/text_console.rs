// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map owned terminal style facts and clip actual shaped geometry to console rows.

use super::PaintSpec;
use crate::bitmap::Bitmap;
use std::{io, num::NonZeroUsize};
use sugarsubstitute_contracts::configuration::Theme;
use sugarsubstitute_contracts::execution::MemoryLease;
#[cfg(test)]
use sugarsubstitute_contracts::transcript::AnsiColor;
use sugarsubstitute_contracts::transcript::TextStyle;

/// Retain validated UTF-8/UTF-16 boundaries after bounded row projection.
pub struct ProjectedRun {
    /// Inclusive UTF-8 byte offset.
    pub start: u32,
    /// Exclusive UTF-8 byte offset.
    pub end: u32,
    /// Inclusive UTF-16 code-unit offset for native shaping.
    pub utf16_start: u32,
    /// Exclusive UTF-16 code-unit offset for native shaping.
    pub utf16_end: u32,
    /// Source-owned terminal colors and emphasis.
    pub style: TextStyle,
}

/// Borrow one projected row only through the actual synchronous shaping call.
pub struct ConsoleRowView<'a> {
    /// Valid UTF-8 projected text, bounded during shaping.
    pub text: &'a str,
    /// Bounded ranges into this text, preserved through the shaping call.
    pub runs: &'a [ProjectedRun],
}

/// Bind one displayed record to the exact surface and half-open console clip.
#[derive(Clone, Copy)]
pub struct ConsolePaintSpec {
    /// Device target, scale and terminal palette.
    pub surface: PaintSpec,
    /// Logical top-left of this row.
    pub origin: [f32; 2],
    /// Logical row width and line height.
    pub extent: [f32; 2],
    /// Half-open logical console clipping rectangle.
    pub bounds: [f32; 4],
    /// Font em size in logical pixels.
    pub font_size: f32,
}
impl ConsolePaintSpec {
    /// Validate one row before shaping or destination mutation.
    /// # Errors
    /// Rejects invalid target, font and row geometry.
    pub fn validate(self) -> io::Result<()> {
        self.label().validate("")
    }
    /// Logical top of the current row.
    pub fn top(self) -> f32 {
        self.origin[1]
    }
    /// Half-open device coordinates for the current console clip.
    pub fn clip(self) -> [i32; 4] {
        self.bounds.map(|v| (v * self.surface.scale).round() as i32)
    }
    /// Use the same label shaping and raster owner for styled console rows.
    pub(super) fn label(self) -> super::LabelPaintSpec {
        super::LabelPaintSpec {
            surface: self.surface,
            layout: super::LabelLayout {
                origin: self.origin,
                extent: self.extent,
                clip: Some(self.bounds),
                anchor: super::VerticalAnchor::Top,
                alignment: super::TextAlignment::Leading,
                metrics: super::TextMetrics::GdiCompatible,
                font_size: self.font_size,
                line_height: self.extent[1],
                weight: 400,
                color: if self.surface.theme == Theme::Dark { [255; 4] } else { [0, 0, 0, 255] },
            },
        }
    }
}

/// External font/shaping boundary; projection and row sequencing remain with the console owner.
pub trait ConsoleFont {
    /// Owned bounded raster cursor; it does not borrow this font owner.
    type Paint: ConsoleGlyph;
    /// Shape a validated row on its preparation worker.
    /// # Errors
    /// Preserves invalid input, memory admission and font failures. A missing
    /// lease is reserved for synchronous offline qualification.
    fn prepare_console(
        &mut self,
        row: ConsoleRowView<'_>,
        spec: ConsolePaintSpec,
        lease: Option<MemoryLease>,
    ) -> io::Result<Self::Paint>;
}

/// External glyph cursor advances one bounded native/raster operation on owned plans.
pub trait ConsoleGlyph {
    /// Advance at most the requested authored pixel budget or one bounded native call.
    /// # Errors
    /// Preserves target identity, allocation and raster failures.
    fn step(&mut self, target: &mut Bitmap, maximum: NonZeroUsize) -> io::Result<bool>;
}

/// Copy one actual shaper range rectangle before its native/layout owner returns.
pub struct Background {
    /// Half-open logical bounds supplied by the actual text shaper.
    pub bounds: [f32; 4],
    /// Unpremultiplied sRGB source background.
    pub color: [u8; 4],
}

/// Composite bounded background spans before glyphs without modifying foreign text.
pub struct BackgroundPaint {
    spans: Vec<Background>,
    span: usize,
    position: usize,
    spec: ConsolePaintSpec,
}
impl BackgroundPaint {
    /// Validate actual bounded geometry before retaining it for cooperative painting.
    pub fn new(spans: Vec<Background>, spec: ConsolePaintSpec) -> io::Result<Self> {
        spec.validate()?;
        if spans.len() > 4096
            || spans.capacity() > 4096
            || spans.iter().any(|s| {
                s.bounds.iter().any(|v| !v.is_finite())
                    || s.bounds[2] < s.bounds[0]
                    || s.bounds[3] < s.bounds[1]
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid shaped console backgrounds",
            ));
        }
        Ok(Self { spans, span: 0, position: 0, spec })
    }
    /// Visit at most32768 clipped device pixels, preserving all pixels outside the row.
    pub fn step(&mut self, target: &mut Bitmap, maximum: NonZeroUsize) -> io::Result<bool> {
        if target.width() != self.spec.surface.width || target.height() != self.spec.surface.height
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Console target shape changed",
            ));
        }
        let mut remaining = maximum.get().min(32768);
        let mut inspected = 0;
        let clip = self.spec.clip();
        let scale = self.spec.surface.scale;
        while self.span < self.spans.len() && remaining > 0 && inspected < 256 {
            inspected += 1;
            let span = &self.spans[self.span];
            let left = (span.bounds[0] * scale).floor().max(clip[0].max(0) as f32) as usize;
            let top = (span.bounds[1] * scale)
                .floor()
                .max((self.spec.top() * scale).round())
                .max(clip[1].max(0) as f32) as usize;
            let right = (span.bounds[2] * scale)
                .ceil()
                .min(clip[2].min(target.width() as i32) as f32)
                .max(left as f32) as usize;
            let bottom = (span.bounds[3] * scale)
                .ceil()
                .min(((self.spec.top() + self.spec.extent[1]) * scale).round())
                .min(clip[3].min(target.height() as i32) as f32)
                .max(top as f32) as usize;
            let width = right - left;
            let pixels = width * (bottom - top);
            let end = self.position.saturating_add(remaining).min(pixels);
            for n in self.position..end {
                let x = left + n % width;
                let y = top + n / width;
                let p = (y * target.width() as usize + x) * 4;
                composite(&mut target.data_mut()[p..p + 4], span.color, 255);
            }
            remaining -= end - self.position;
            self.position = end;
            if end == pixels {
                self.span += 1;
                self.position = 0;
            }
        }
        Ok(self.span == self.spans.len())
    }
}

/// Preserve premultiplied RGBA while compositing one owned foreground/background color.
pub(super) fn composite(pixel: &mut [u8], color: [u8; 4], coverage: u8) {
    let multiply = |a: u8, b: u8| {
        let p = u32::from(a) * u32::from(b) + 128;
        ((p + (p >> 8)) >> 8) as u8
    };
    let alpha = multiply(coverage, color[3]);
    for (channel, value) in pixel.iter_mut().take(3).zip(color) {
        *channel = multiply(value, alpha).saturating_add(multiply(*channel, 255 - alpha));
    }
    pixel[3] = alpha.saturating_add(multiply(pixel[3], 255 - alpha));
}

#[cfg(test)]
#[path = "text_console_tests.rs"]
mod tests;
