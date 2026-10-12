// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map owned terminal style facts and clip actual shaped geometry to console rows.

use super::memory::PaintSpec;
use crate::Theme;
use std::{io, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_contracts::transcript::{AnsiColor, TextStyle};
use tiny_skia::Pixmap;

/// Retain validated UTF-8/UTF-16 boundaries after bounded row projection.
pub(crate) struct ProjectedRun {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) utf16_start: u32,
    pub(crate) utf16_end: u32,
    pub(crate) style: TextStyle,
}

/// Borrow one projected row only through the actual synchronous shaping call.
pub(crate) struct ConsoleRowView<'a> {
    pub(crate) text: &'a str,
    pub(crate) runs: &'a [ProjectedRun],
}

/// Bind one displayed record to the exact surface and half-open console clip.
#[derive(Clone, Copy)]
pub(crate) struct ConsolePaintSpec {
    pub(crate) surface: PaintSpec,
    pub(crate) row: usize,
    pub(crate) rows: usize,
}
impl ConsolePaintSpec {
    /// Bind validated logical row coordinates to one actual immutable surface.
    pub(crate) fn new(
        theme: Theme,
        scale: f32,
        width: u32,
        height: u32,
        row: usize,
        rows: usize,
    ) -> io::Result<Self> {
        let value = Self { surface: PaintSpec { theme, scale, width, height }, row, rows };
        value.validate()?;
        Ok(value)
    }
    /// Reject geometry before shaping or any target mutation.
    pub(crate) fn validate(self) -> io::Result<()> {
        if self.rows == 0 || self.rows > 10 || self.row >= self.rows {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid console row geometry",
            ));
        }
        super::memory::validate("", self.surface.scale, self.surface.width, self.surface.height)
    }
    /// Align the latest row with the bottom of the original console inner rectangle.
    pub(crate) fn top(self) -> f32 {
        528.0 - (self.rows - self.row) as f32 * 16.0
    }
    /// Return half-open device coordinates matching the original console inner clip.
    pub(crate) fn clip(self) -> [i32; 4] {
        let s = self.surface.scale;
        [30.0, 362.0, 528.0, 528.0].map(|v| (v * s).round() as i32)
    }
}

/// External font/shaping boundary; projection and row sequencing remain with the console owner.
pub(crate) trait ConsoleFont {
    type Paint: ConsoleGlyph;
    fn prepare_console(
        &mut self,
        row: ConsoleRowView<'_>,
        spec: ConsolePaintSpec,
        lease: Option<MemoryLease>,
    ) -> io::Result<Self::Paint>;
}

/// External glyph cursor advances one bounded native/raster operation on owned plans.
pub(crate) trait ConsoleGlyph {
    fn step(&mut self, target: &mut Pixmap, maximum: NonZeroUsize) -> io::Result<bool>;
}

/// Preserve the original terminal's basic palette; extended colors follow ANSI indices.
pub(crate) fn color(value: AnsiColor, theme: Theme, background: bool) -> Option<[u8; 3]> {
    const BASIC: [[u8; 3]; 16] = [
        [0x17, 0x19, 0x1c],
        [0xf6, 0x61, 0x51],
        [0x33, 0xd1, 0x7a],
        [0xe5, 0xa5, 0x0a],
        [0x35, 0x84, 0xe4],
        [0xc0, 0x61, 0xcb],
        [0x33, 0xc7, 0xde],
        [0xde, 0xdd, 0xda],
        [0x77, 0x76, 0x7b],
        [0xff, 0x7b, 0x72],
        [0x57, 0xe3, 0x89],
        [0xf8, 0xe4, 0x5c],
        [0x62, 0xa0, 0xea],
        [0xdc, 0x8a, 0xdd],
        [0x5b, 0xc8, 0xaf],
        [0xff, 0xff, 0xff],
    ];
    Some(match value {
        AnsiColor::Default if background => return None,
        AnsiColor::Default => {
            if theme == Theme::Dark {
                [255; 3]
            } else {
                [0; 3]
            }
        }
        AnsiColor::Rgb(r, g, b) => [r, g, b],
        AnsiColor::Indexed(i @ 0..=15) => BASIC[usize::from(i)],
        AnsiColor::Indexed(i @ 16..=231) => {
            let n = i - 16;
            let component = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            [component(n / 36), component((n / 6) % 6), component(n % 6)]
        }
        AnsiColor::Indexed(i) => [8 + 10 * (i - 232); 3],
    })
}

/// Copy one actual shaper range rectangle before its native/layout owner returns.
pub(crate) struct Background {
    pub(crate) bounds: [f32; 4],
    pub(crate) color: [u8; 3],
}

/// Composite bounded background spans before glyphs without modifying foreign text.
pub(crate) struct BackgroundPaint {
    spans: Vec<Background>,
    span: usize,
    position: usize,
    spec: ConsolePaintSpec,
}
impl BackgroundPaint {
    /// Validate actual bounded geometry before retaining it for cooperative painting.
    pub(crate) fn new(spans: Vec<Background>, spec: ConsolePaintSpec) -> io::Result<Self> {
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
    pub(crate) fn step(&mut self, target: &mut Pixmap, maximum: NonZeroUsize) -> io::Result<bool> {
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
                .min(((self.spec.top() + 16.0) * scale).round())
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
pub(crate) fn composite(pixel: &mut [u8], color: [u8; 3], alpha: u8) {
    let multiply = |a: u8, b: u8| {
        let p = u32::from(a) * u32::from(b) + 128;
        ((p + (p >> 8)) >> 8) as u8
    };
    for (channel, value) in pixel.iter_mut().take(3).zip(color) {
        *channel = multiply(value, alpha).saturating_add(multiply(*channel, 255 - alpha));
    }
    pixel[3] = alpha.saturating_add(multiply(pixel[3], 255 - alpha));
}

#[cfg(test)]
#[path = "text_console_tests.rs"]
mod tests;
