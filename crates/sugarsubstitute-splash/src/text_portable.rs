// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shape bounded portable labels without retaining glyph images across frame scales.
//!
//! Font discovery and opaque library shaping remain worker-owned operations.
//! Their internal allocations are not represented as known Rust raster capacities.

use crate::{
    Theme,
    text::memory::{self, Allocation, MAX_GLYPHS, METADATA_BYTES},
};
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, fontdb};
use std::{
    io,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::MemoryLease;

#[path = "text_portable_cursor.rs"]
mod cursor;
use cursor::Glyph;
pub(crate) use cursor::StatusPaint;

/// Retain font discovery while each cursor owns its uncached glyph image lifetime.
pub(crate) struct StatusText {
    fonts: Arc<Mutex<FontSystem>>,
}

impl StatusText {
    /// Discover installed fonts on the preparation worker after bootstrap.
    pub(crate) fn new(locale: &str) -> io::Result<Self> {
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

    /// Prepare owned glyph plans using a real admitted reservation before allocations.
    ///
    /// Library shaping is indivisible, with at most 4096 UTF-16 units and glyphs.
    /// No destination is borrowed; opaque font/scaler allocations remain separate
    /// from the explicitly claimed plan/image capacities.
    pub(crate) fn prepare(
        &mut self,
        message: &str,
        theme: Theme,
        scale: f32,
        width: u32,
        height: u32,
        lease: MemoryLease,
    ) -> io::Result<StatusPaint> {
        self.begin(message, theme, scale, width, height, Allocation(Some(lease)))
    }

    /// Prepare the same cursor for synchronous offline qualification without a lease.
    pub(crate) fn prepare_offline(
        &mut self,
        message: &str,
        theme: Theme,
        scale: f32,
        width: u32,
        height: u32,
    ) -> io::Result<StatusPaint> {
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
    ) -> io::Result<StatusPaint> {
        memory::validate(message, scale, width, height)?;
        let metadata = allocation.claim(METADATA_BYTES)?;
        let mut plans = memory::reserve::<Glyph>(MAX_GLYPHS, METADATA_BYTES)?;
        let mut fonts = cursor::fonts(&self.fonts)?;
        let mut buffer = Buffer::new(&mut fonts, Metrics::new(14.0 * scale, 16.0 * scale));
        buffer.set_size(Some(510.0 * scale), Some(36.0 * scale));
        buffer.set_text(message, &Attrs::new().family(Family::SansSerif), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut fonts, false);
        let origin_y =
            buffer.layout_runs().next().map_or(504.5 * scale, |run| 519.0 * scale - run.line_y);
        let color =
            if theme == Theme::Dark { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) };
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
        drop(fonts);
        Ok(StatusPaint::new(
            self.fonts.clone(),
            plans,
            allocation,
            metadata,
            memory::PaintSpec { theme, scale, width, height },
            origin_y,
        ))
    }
}

#[cfg(test)]
#[path = "text_portable_tests.rs"]
mod tests;
