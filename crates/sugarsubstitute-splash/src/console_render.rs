// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project at most ten immutable transcript rows and serialize their actual glyph cursors.

use crate::Theme;
use std::{io, num::NonZeroUsize};
use sugarsubstitute_application::transcript::TranscriptSnapshot;
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    transcript::{TextStyle, TranscriptRecord},
};
use sugarsubstitute_ui::bitmap::Bitmap;
use sugarsubstitute_ui::text::console::{ConsoleFont, ConsoleGlyph, ConsoleRowView, ProjectedRun};

const PROJECTION_BYTES: usize = 1024 * 1024;
const MAX_UTF16: usize = 4096;
const MAX_STYLES: usize = 256;

/// Retain bounded drawing text without copying or changing complete transcript history.
struct Projection {
    text: String,
    runs: Vec<ProjectedRun>,
    offset: usize,
    utf16: usize,
    style: usize,
    style_end: usize,
    current_style: Option<TextStyle>,
    done: bool,
    clipped: bool,
    // Owned projection buffers drop before their admitted byte claim.
    _claim: Option<MemoryClaim>,
}
impl Projection {
    fn new(lease: Option<&MemoryLease>) -> io::Result<Self> {
        let claim = lease
            .map(|l| l.claim(PROJECTION_BYTES as u64).map_err(io::Error::other))
            .transpose()?;
        let mut text = String::new();
        text.try_reserve_exact(MAX_UTF16 * 4).map_err(io::Error::other)?;
        let mut runs = Vec::new();
        runs.try_reserve_exact(MAX_STYLES).map_err(io::Error::other)?;
        if text.capacity() + runs.capacity() * size_of::<ProjectedRun>() > PROJECTION_BYTES {
            return Err(io::Error::other("Console projection exceeds admitted capacity"));
        }
        Ok(Self {
            text,
            runs,
            offset: 0,
            utf16: 0,
            style: 0,
            style_end: 0,
            current_style: None,
            done: false,
            clipped: false,
            _claim: claim,
        })
    }
    /// Inspect at most4096 UTF8 bytes; stop at a valid character/style drawing bound.
    fn step(&mut self, record: &TranscriptRecord) -> io::Result<bool> {
        if self.done {
            return Ok(true);
        }
        let initial = self.offset;
        let mut styles = 0;
        while self.offset < record.text().len() {
            let character = record.text()[self.offset..]
                .chars()
                .next()
                .ok_or_else(|| io::Error::other("Console character absent"))?;
            let bytes = character.len_utf8();
            if self.offset - initial + bytes > 4096 {
                return Ok(false);
            }
            if self.offset >= self.style_end {
                if styles == MAX_STYLES {
                    return Ok(false);
                }
                let run = record
                    .runs()
                    .get(self.style)
                    .ok_or_else(|| io::Error::other("Console style absent"))?;
                self.style_end = run.end as usize;
                self.current_style = Some(run.style);
                self.style += 1;
                styles += 1;
            }
            let style = self
                .current_style
                .ok_or_else(|| io::Error::other("Console current style absent"))?;
            let changing = self.runs.last().is_none_or(|run| run.style != style);
            if self.utf16 + character.len_utf16() > MAX_UTF16
                || (changing && self.runs.len() == MAX_STYLES)
            {
                self.done = true;
                self.clipped = true;
                return Ok(true);
            }
            let start = self.text.len();
            let utf16_start = self.utf16;
            self.text.push(character);
            self.offset += bytes;
            self.utf16 += character.len_utf16();
            if changing {
                self.runs.push(ProjectedRun {
                    start: start as u32,
                    end: self.text.len() as u32,
                    utf16_start: utf16_start as u32,
                    utf16_end: self.utf16 as u32,
                    style,
                });
            } else if let Some(run) = self.runs.last_mut() {
                run.end = self.text.len() as u32;
                run.utf16_end = self.utf16 as u32;
            }
        }
        self.done = true;
        Ok(true)
    }
    fn view(&self) -> ConsoleRowView<'_> {
        ConsoleRowView { text: &self.text, runs: &self.runs }
    }
}

/// Retain drawing cursors only; the accepted task owns the complete snapshot through disposal.
pub(crate) struct ConsoleRender<P: ConsoleGlyph> {
    first: usize,
    rows: usize,
    row: usize,
    theme: Theme,
    scale: f32,
    width: u32,
    height: u32,
    projection: Option<Projection>,
    glyph: Option<P>,
    clipped_rows: u8,
}
impl<P: ConsoleGlyph> ConsoleRender<P> {
    /// Capture bounded selection indices without acquiring or copying any history pins.
    pub(crate) fn new(
        snapshot: &TranscriptSnapshot,
        theme: Theme,
        scale: f32,
        width: u32,
        height: u32,
    ) -> Self {
        let count = snapshot.records().len();
        let rows = count.min(10);
        Self {
            first: count - rows,
            rows,
            row: 0,
            theme,
            scale,
            width,
            height,
            projection: None,
            glyph: None,
            clipped_rows: 0,
        }
    }
    /// Report actual drawing-bound clipping for at most ten projected rows.
    pub(crate) const fn clipped_rows(&self) -> u8 {
        self.clipped_rows
    }
    /// Advance projection, one actual shaping call, or one <=32768-pixel glyph slice.
    pub(crate) fn step<F: ConsoleFont<Paint = P>>(
        &mut self,
        snapshot: &TranscriptSnapshot,
        fonts: &mut F,
        target: &mut Bitmap,
        lease: Option<MemoryLease>,
    ) -> io::Result<bool> {
        if self.row == self.rows {
            return Ok(true);
        }
        if let Some(glyph) = &mut self.glyph {
            if glyph.step(
                target,
                NonZeroUsize::new(32768)
                    .ok_or_else(|| io::Error::other("Console pixel budget absent"))?,
            )? {
                self.glyph = None;
                self.row += 1;
            }
            return Ok(self.row == self.rows);
        }
        let record = snapshot
            .records()
            .nth(self.first + self.row)
            .ok_or_else(|| io::Error::other("Original console row absent"))?;
        if self.projection.is_none() {
            self.projection = Some(Projection::new(lease.as_ref())?);
            return Ok(false);
        }
        let projection = self
            .projection
            .as_mut()
            .ok_or_else(|| io::Error::other("Console projection absent"))?;
        if !projection.step(record)? {
            return Ok(false);
        }
        if projection.clipped {
            self.clipped_rows = self
                .clipped_rows
                .checked_add(1)
                .filter(|count| *count <= 10)
                .ok_or_else(|| io::Error::other("Console clipped-row invariant failed"))?;
        }
        if projection.text.is_empty() {
            self.projection = None;
            self.row += 1;
            return Ok(self.row == self.rows);
        }
        let spec = crate::text_layout::console(
            self.theme,
            self.scale,
            self.width,
            self.height,
            self.row,
            self.rows,
        )?;
        self.glyph = Some(fonts.prepare_console(projection.view(), spec, lease)?);
        self.projection = None;
        Ok(false)
    }
}

#[cfg(test)]
#[path = "console_render_tests.rs"]
mod tests;
