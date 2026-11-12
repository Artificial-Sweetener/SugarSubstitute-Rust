// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own native Windows font shaping and bounded grayscale glyph coverage.
//!
//! DirectWrite objects and copied callback buffers remain owned by a worker's
//! preparation cursor. Layout Draw is synchronous and limited to 4096 UTF-16
//! units, 256 runs and 4096 glyphs. Shared-factory operations require no HWND or
//! COM apartment. Opaque native font storage is outside the Rust allocation claim.

use crate::bitmap::Bitmap;
use crate::text::LabelPaintSpec;
use std::{cell::RefCell, io};
use sugarsubstitute_contracts::execution::MemoryLease;
use windows::{
    Win32::Graphics::DirectWrite::*,
    core::{ComObject, Interface, PCWSTR, w},
};

#[path = "text_windows_runs.rs"]
mod runs;
use super::memory::{self, Allocation, MAX_GLYPHS, MAX_RUNS, METADATA_BYTES};
use runs::{Collector, GlyphRun};
#[path = "text_windows_cursor.rs"]
mod cursor;
pub use cursor::GlyphPaint;
#[path = "text_windows_console.rs"]
mod console;
#[path = "text_windows_layout.rs"]
mod layout;

/// Retain native font discovery for worker-owned native label preparation.
pub struct FontOwner {
    factory: IDWriteFactory2,
    format: IDWriteTextFormat,
}

impl FontOwner {
    /// Prepare the installed Segoe UI family with locale-aware system fallback.
    /// # Errors
    /// Preserves invalid locale and native font discovery failures.
    pub fn new(locale: &str) -> Result<Self, io::Error> {
        if locale.len() > 64 || locale.contains('\0') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid status font locale"));
        }
        let mut native_locale = memory::reserve::<u16>(65, 130)?;
        native_locale.extend(locale.encode_utf16().chain(Some(0)));
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
                PCWSTR(native_locale.as_ptr()),
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

    /// Shape a label on its admitted preparation worker. Native shaping is bounded
    /// but indivisible; cancellation is observed when the call returns.
    /// # Errors
    /// Preserves invalid input, allocation, font discovery and shaping failures.
    pub fn prepare(
        &mut self,
        message: &str,
        spec: LabelPaintSpec,
        lease: MemoryLease,
    ) -> io::Result<GlyphPaint> {
        self.begin(message, spec, Allocation(Some(lease)))
    }

    /// Shape the same label synchronously for offline raster qualification.
    /// Production jobs use `prepare` and retain their admitted memory claims.
    /// # Errors
    /// Preserves invalid input, allocation and native shaping failures.
    pub fn prepare_offline(
        &mut self,
        message: &str,
        spec: LabelPaintSpec,
    ) -> io::Result<GlyphPaint> {
        self.begin(message, spec, Allocation(None))
    }

    fn begin(
        &self,
        message: &str,
        spec: LabelPaintSpec,
        allocation: Allocation,
    ) -> io::Result<GlyphPaint> {
        spec.validate(message)?;
        let metadata = allocation.claim(METADATA_BYTES)?;
        let runs = self.shape(message, spec)?;
        Ok(GlyphPaint::new(self.factory.clone(), runs, metadata, allocation, spec))
    }

    /// Copy native shaping buffers within the already admitted metadata allowance.
    fn shape(&self, message: &str, spec: LabelPaintSpec) -> io::Result<Vec<GlyphRun>> {
        if message.encode_utf16().take(MAX_GLYPHS + 1).count() > MAX_GLYPHS {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Status exceeds UTF-16 bound"));
        }
        if message.is_empty() {
            return Ok(Vec::new());
        }
        let layout = self.layout(message, spec)?;
        let runs = memory::reserve::<GlyphRun>(MAX_RUNS, METADATA_BYTES / 2)?;
        let collector = ComObject::new(Collector {
            runs: RefCell::new(runs),
            failure: RefCell::new(None),
            colors: Vec::new(),
        });
        let renderer: IDWriteTextRenderer = collector.to_interface();
        // SAFETY: The bounded UTF-16 slice and format remain valid through layout
        // creation. Draw is synchronous and retains the renderer during callbacks.
        unsafe {
            let result = layout.Draw(None, &renderer, 0.0, 0.0);
            if let Some(cause) = collector.get().failure.replace(None) {
                return Err(cause);
            }
            result?;
        }
        Ok(collector.get().runs.replace(Vec::new()))
    }
}
/// Borrow the destination with the QLabel's half-open device-pixel clip rectangle.
pub(super) struct LabelSurface<'a> {
    pixels: &'a mut Bitmap,
    bounds: [i32; 4],
}

impl LabelSurface<'_> {
    /// Clip font overhang before compositing while preserving premultiplication.
    fn composite(&mut self, x: i32, y: i32, alpha: u8, color: [u8; 4]) {
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
        crate::text::console::composite(pixel, color, alpha);
    }
}

#[cfg(test)]
#[path = "text_windows_tests.rs"]
mod tests;
