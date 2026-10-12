// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own native Windows font shaping and bounded grayscale glyph coverage.
//!
//! DirectWrite objects and copied callback buffers remain owned by a worker's
//! preparation cursor. Layout Draw is synchronous and limited to 4096 UTF-16
//! units, 256 runs and 4096 glyphs. Shared-factory operations require no HWND or
//! COM apartment. Opaque native font storage is outside the Rust allocation claim.

use std::{cell::RefCell, io};
use sugarsubstitute_contracts::execution::MemoryLease;
use tiny_skia::Pixmap;
use windows::{
    Win32::Graphics::DirectWrite::*,
    core::{ComObject, Interface, PCWSTR, w},
};

use crate::Theme;

#[path = "text_windows_runs.rs"]
mod runs;
use super::memory::{self, Allocation, MAX_GLYPHS, MAX_RUNS, METADATA_BYTES};
use runs::{Collector, GlyphRun};
#[path = "text_windows_cursor.rs"]
mod cursor;
pub(crate) use cursor::GlyphPaint;
#[path = "text_windows_console.rs"]
mod console;

/// Retain native font discovery after the first splash submission.
pub(crate) struct FontOwner {
    factory: IDWriteFactory2,
    format: IDWriteTextFormat,
}

impl FontOwner {
    /// Prepare the installed Segoe UI family with locale-aware system fallback.
    pub(crate) fn new(locale: &str) -> Result<Self, io::Error> {
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

    /// Prepare an owned cooperative cursor against an actual admitted reservation.
    ///
    /// Font creation and this bounded native shaping call run on the preparation
    /// worker. Rust metadata claims precede allocations; no destination is touched.
    /// Native layout is indivisible and cancellation is observed after it returns.
    pub(crate) fn prepare_status(
        &mut self,
        message: &str,
        theme: Theme,
        scale: f32,
        width: u32,
        height: u32,
        lease: MemoryLease,
    ) -> io::Result<crate::text::GlyphPaint> {
        self.begin(message, theme, scale, width, height, Allocation(Some(lease)))
    }

    /// Prepare the same cursor for synchronous offline raster qualification.
    ///
    /// This path owns allocations without an execution reservation. Production
    /// frame jobs use `prepare_status` and keep their real byte claims with the cursor.
    pub(crate) fn prepare_status_offline(
        &mut self,
        message: &str,
        theme: Theme,
        scale: f32,
        width: u32,
        height: u32,
    ) -> io::Result<GlyphPaint> {
        self.begin(message, theme, scale, width, height, Allocation(None))
    }

    fn begin(
        &self,
        message: &str,
        theme: Theme,
        scale: f32,
        width: u32,
        height: u32,
        allocation: Allocation,
    ) -> io::Result<GlyphPaint> {
        memory::validate(message, scale, width, height)?;
        let metadata = allocation.claim(METADATA_BYTES)?;
        let runs = self.shape(message)?;
        Ok(GlyphPaint::new(
            self.factory.clone(),
            runs,
            metadata,
            allocation,
            memory::PaintSpec { theme, scale, width, height },
        ))
    }

    /// Copy native shaping buffers within the already admitted metadata allowance.
    fn shape(&self, message: &str) -> io::Result<Vec<GlyphRun>> {
        if message.encode_utf16().take(MAX_GLYPHS + 1).count() > MAX_GLYPHS {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Status exceeds UTF-16 bound"));
        }
        if message.is_empty() {
            return Ok(Vec::new());
        }
        let mut text = memory::reserve::<u16>(MAX_GLYPHS, METADATA_BYTES)?;
        text.extend(message.encode_utf16());
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
            let layout = self.factory.CreateGdiCompatibleTextLayout(
                &text,
                &self.format,
                510.0,
                36.0,
                1.0,
                None,
                false,
            )?;
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
    pixels: &'a mut Pixmap,
    bounds: [i32; 4],
}

impl LabelSurface<'_> {
    /// Clip font overhang before compositing while preserving premultiplication.
    fn composite(&mut self, x: i32, y: i32, alpha: u8, color: [u8; 3]) {
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
