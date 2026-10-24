// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shape bounded portable labels without retaining glyph images across frame scales.
//!
//! Font discovery and opaque library shaping remain worker-owned operations.
//! Their internal allocations are not represented as known Rust raster capacities.

use crate::text::memory::{self, Allocation, MAX_GLYPHS, METADATA_BYTES};
use crate::text::{LabelPaintSpec, TextAlignment, VerticalAnchor};
use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Weight, fontdb,
};
use std::{
    io,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::MemoryLease;

#[path = "text_portable_cursor.rs"]
mod cursor;
use cursor::Glyph;
pub use cursor::GlyphPaint;
#[path = "text_portable_console.rs"]
mod console;

/// Retain font discovery while each cursor owns its uncached glyph image lifetime.
pub struct FontOwner {
    fonts: Arc<Mutex<FontSystem>>,
}

impl FontOwner {
    /// Discover installed fonts on the preparation worker.
    /// # Errors
    /// Preserves invalid locales and missing platform font discovery.
    pub fn new(locale: &str) -> io::Result<Self> {
        if locale.len() > 64 || locale.contains('\0') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid status font locale"));
        }
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        if database.faces().next().is_none() {
            return Err(io::Error::other("No usable platform fonts are available"));
        }
        database.set_sans_serif_family("sans-serif");
        Ok(Self {
            fonts: Arc::new(Mutex::new(FontSystem::new_with_locale_and_db(
                locale.to_owned(),
                database,
            ))),
        })
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

    /// Measure every shaped line before viewport clipping on the preparation worker.
    /// Production callers supply admitted storage; `None` is for offline qualification.
    /// # Errors
    /// Preserves invalid input, allocation, admission and font-shaping failures.
    pub fn measure(
        &mut self,
        message: &str,
        spec: LabelPaintSpec,
        lease: Option<&MemoryLease>,
    ) -> io::Result<crate::text::TextSize> {
        spec.validate(message)?;
        let _claim = lease
            .map(|lease| lease.claim(METADATA_BYTES as u64))
            .transpose()
            .map_err(io::Error::other)?;
        if message.is_empty() {
            return Ok(crate::text::TextSize::default());
        }
        let buffer = self.layout(message, spec)?;
        let mut size = crate::text::TextSize::default();
        for run in buffer.layout_runs() {
            size.width = size.width.max(run.line_w / spec.surface.scale);
            size.height = size.height.max((run.line_top + run.line_height) / spec.surface.scale);
            size.lines += 1;
        }
        Ok(size)
    }

    /// Shape once with the same width, typography and alignment for measure and paint.
    fn layout(&self, message: &str, spec: LabelPaintSpec) -> io::Result<Buffer> {
        spec.validate(message)?;
        let mut fonts = cursor::fonts(&self.fonts)?;
        let scale = spec.surface.scale;
        let layout = spec.layout;
        let mut buffer = Buffer::new(
            &mut fonts,
            Metrics::new(layout.font_size * scale, layout.line_height * scale),
        );
        buffer.set_size(Some(layout.extent[0] * scale), None);
        buffer.set_text(
            message,
            &Attrs::new().family(Family::SansSerif).weight(Weight(layout.weight)),
            Shaping::Advanced,
            None,
        );
        let align = match layout.alignment {
            TextAlignment::Leading => None,
            TextAlignment::Center => Some(Align::Center),
            TextAlignment::Trailing => Some(Align::End),
        };
        for line in &mut buffer.lines {
            line.set_align(align);
        }
        buffer.shape_until_scroll(&mut fonts, false);
        Ok(buffer)
    }

    fn begin(
        &self,
        message: &str,
        spec: LabelPaintSpec,
        allocation: Allocation,
    ) -> io::Result<GlyphPaint> {
        spec.validate(message)?;
        let metadata = allocation.claim(METADATA_BYTES)?;
        let mut plans = memory::reserve::<Glyph>(MAX_GLYPHS, METADATA_BYTES)?;
        let scale = spec.surface.scale;
        let layout = spec.layout;
        let buffer = self.layout(message, spec)?;
        let baseline = match layout.anchor {
            VerticalAnchor::Top => 0.0,
            VerticalAnchor::FirstBaseline => buffer
                .layout_runs()
                .next()
                .map_or((layout.font_size + 0.5) * scale, |run| run.line_y),
            VerticalAnchor::LastBaseline => buffer
                .layout_runs()
                .last()
                .map_or((layout.font_size + 0.5) * scale, |run| run.line_y),
        };
        let origin_y = layout.origin[1] * scale - baseline;
        let [r, g, b, a] = layout.color;
        let color = Color::rgba(r, g, b, a);
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                if plans.len() == MAX_GLYPHS {
                    return Err(io::Error::other("Status exceeds shaped glyph bound"));
                }
                plans.push(Glyph {
                    physical: glyph.physical((0.0, run.line_y), 1.0),
                    color: glyph.color_opt.unwrap_or(color),
                });
            }
        }
        drop(buffer);
        Ok(GlyphPaint::new(self.fonts.clone(), plans, allocation, metadata, spec, origin_y))
    }
}

#[cfg(test)]
#[path = "text_portable_tests.rs"]
mod tests;
