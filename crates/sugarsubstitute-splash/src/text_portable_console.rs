// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shape the projected console through the actual portable font and cluster owner.

use super::{
    FontOwner, GlyphPaint,
    cursor::{self, Glyph},
};
use crate::text::{
    console::{
        Background, BackgroundPaint, ConsoleFont, ConsoleGlyph, ConsolePaintSpec, ConsoleRowView,
    },
    memory::{self, Allocation, MAX_GLYPHS, MAX_RUNS, METADATA_BYTES},
};
use cosmic_text::{Attrs, Buffer, Color, Family, Metrics, Shaping, Weight, Wrap};
use std::{io, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_contracts::presentation::console_color;
use tiny_skia::Pixmap;

impl ConsoleFont for FontOwner {
    type Paint = GlyphPaint;
    fn prepare_console(
        &mut self,
        row: ConsoleRowView<'_>,
        spec: ConsolePaintSpec,
        lease: Option<MemoryLease>,
    ) -> io::Result<GlyphPaint> {
        spec.validate()?;
        let surface = spec.surface;
        memory::validate(row.text, surface.scale, surface.width, surface.height)?;
        if row.runs.len() > MAX_RUNS
            || row.runs.iter().any(|run| {
                run.start >= run.end || row.text.get(run.start as usize..run.end as usize).is_none()
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid projected console ranges",
            ));
        }
        let allocation = Allocation(lease);
        let metadata = allocation.claim(METADATA_BYTES)?;
        let mut plans = memory::reserve::<Glyph>(MAX_GLYPHS, METADATA_BYTES / 2)?;
        let mut backgrounds = memory::reserve::<Background>(MAX_GLYPHS, METADATA_BYTES / 4)?;
        let mut fonts = cursor::fonts(&self.fonts)?;
        let mut buffer =
            Buffer::new(&mut fonts, Metrics::new(14.0 * surface.scale, 16.0 * surface.scale));
        buffer.set_size(Some(498.0 * surface.scale), Some(16.0 * surface.scale));
        buffer.set_wrap(Wrap::None);
        let [r, g, b, a] = console_color(Default::default(), surface.theme, false)
            .ok_or_else(|| io::Error::other("Console foreground absent"))?;
        let default_color = Color::rgba(r, g, b, a);
        let default_attrs = Attrs::new().family(Family::SansSerif).color(default_color);
        let spans = row.runs.iter().enumerate().map(|(index, run)| {
            let [r, g, b, a] =
                console_color(run.style.foreground, surface.theme, false).unwrap_or([0; 4]);
            (
                &row.text[run.start as usize..run.end as usize],
                default_attrs
                    .clone()
                    .color(Color::rgba(r, g, b, a))
                    .weight(if run.style.bold { Weight::BOLD } else { Weight::NORMAL })
                    .metadata(index),
            )
        });
        buffer.set_rich_text(spans, &default_attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut fonts, false);
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                if plans.len() == MAX_GLYPHS {
                    return Err(io::Error::other("Console shaped glyph bound exceeded"));
                }
                let style = row
                    .runs
                    .get(glyph.metadata)
                    .ok_or_else(|| io::Error::other("Console glyph style absent"))?;
                if let Some(color) = console_color(style.style.background, surface.theme, true) {
                    let left = 30.0 + glyph.x / surface.scale;
                    backgrounds.push(Background {
                        bounds: [
                            left,
                            spec.top(),
                            left + glyph.w / surface.scale,
                            spec.top() + 16.0,
                        ],
                        color,
                    });
                }
                plans.push(Glyph {
                    physical: glyph.physical((0.0, run.line_y), 1.0),
                    color: glyph.color_opt.unwrap_or(default_color),
                });
            }
        }
        drop(buffer);
        drop(fonts);
        let background = BackgroundPaint::new(backgrounds, spec)?;
        let mut paint = GlyphPaint::new(
            self.fonts.clone(),
            plans,
            allocation,
            metadata,
            surface,
            spec.top() * surface.scale,
        );
        paint.bind_console(spec, background);
        Ok(paint)
    }
}
impl ConsoleGlyph for GlyphPaint {
    fn step(&mut self, target: &mut Pixmap, maximum: NonZeroUsize) -> io::Result<bool> {
        GlyphPaint::step(self, target, maximum)
    }
}
