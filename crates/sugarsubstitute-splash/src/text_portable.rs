// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Shape the collapsed status with CPU glyphs and keep fonts outside bootstrap work.

use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, fontdb};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

use crate::Theme;

/// Retain a font system and glyph cache only after text presentation is requested.
pub(crate) struct StatusText {
    fonts: FontSystem,
    cache: SwashCache,
}

impl StatusText {
    /// Load installed platform fonts once after text presentation is requested.
    pub(crate) fn new(locale: &str) -> Result<Self, std::io::Error> {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        if database.faces().next().is_none() {
            return Err(std::io::Error::other("No usable platform fonts are available"));
        }
        database.set_sans_serif_family("sans-serif");
        Ok(Self {
            fonts: FontSystem::new_with_locale_and_db(locale.to_owned(), database),
            cache: SwashCache::new(),
        })
    }

    /// Paint the bottom-aligned 14-pixel label without making it an application owner.
    pub(crate) fn paint(
        &mut self,
        target: &mut Pixmap,
        message: &str,
        theme: Theme,
        scale: f32,
    ) -> Result<(), std::io::Error> {
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(14.0 * scale, 16.0 * scale));
        buffer.set_size(Some(510.0 * scale), Some(36.0 * scale));
        let attributes = Attrs::new().family(Family::SansSerif);
        buffer.set_text(message, &attributes, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        let origin_y = buffer.layout_runs().next().map_or(504.5 * scale, |run| {
            // Preserve the reference label's bottom margin and baseline across display scales.
            519.0 * scale - run.line_y
        });
        let color =
            if theme == Theme::Dark { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) };
        buffer.draw(&mut self.fonts, &mut self.cache, color, |x, y, width, height, color| {
            if let Some(rect) = Rect::from_xywh(
                24.0 * scale + x as f32,
                origin_y + y as f32,
                width as f32,
                height as f32,
            ) {
                let mut paint = Paint { anti_alias: false, ..Paint::default() };
                paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
                target.fill_path(
                    &PathBuilder::from_rect(rect),
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        });
        Ok(())
    }
}
