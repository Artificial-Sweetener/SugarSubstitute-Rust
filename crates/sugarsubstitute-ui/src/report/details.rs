// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold the complete report text and its wrapped line heights, measured in bounded slices so a
//! large report never stalls the thread that prepares the surface.
use crate::{
    scene::SceneResources,
    text::{LabelLayout, LabelPaintSpec, PaintSpec, TextAlignment, TextMetrics, VerticalAnchor},
};
use std::io;
use sugarsubstitute_contracts::execution::MemoryLease;

/// Largest report text accepted, matching the incident text attachment bound.
pub(super) const MAX_BYTES: usize = 256 * 1024;
/// Longest single painted line; longer source lines continue on following display lines.
const MAX_LINE_CHARS: usize = 2048;

/// One display line of the report: a byte range of the original text without its newline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetailsLine {
    start: usize,
    end: usize,
}

/// The complete report text and the measured layout of its lines at one width.
pub struct DetailsText {
    text: String,
    lines: Vec<DetailsLine>,
    key: Option<(u32, u32)>,
    offsets: Vec<f32>,
}

impl DetailsText {
    /// Details typography, shared by measurement and painting.
    pub const FONT_SIZE: f32 = 13.0;
    /// Uniform line height of the details text.
    pub const LINE_HEIGHT: f32 = 19.0;

    /// Split the text into display lines. Carriage returns are dropped; NUL is replaced.
    /// # Errors
    /// Rejects text larger than 256 KiB, the incident text attachment bound.
    pub fn new(text: &str) -> io::Result<Self> {
        if text.len() > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Report text exceeds its bound",
            ));
        }
        let text: String = text
            .chars()
            .filter(|c| *c != '\r')
            .map(|c| if c == '\0' { '\u{fffd}' } else { c })
            .collect();
        let mut lines = Vec::new();
        let mut start = 0;
        for source in text.split('\n') {
            let mut line_start = start;
            let mut count = 0;
            for (index, _) in source.char_indices() {
                if count == MAX_LINE_CHARS {
                    lines.push(DetailsLine { start: line_start, end: start + index });
                    line_start = start + index;
                    count = 0;
                }
                count += 1;
            }
            lines.push(DetailsLine { start: line_start, end: start + source.len() });
            start += source.len() + 1;
        }
        while lines.len() > 1 && lines.last().is_some_and(|line| line.start == line.end) {
            lines.pop();
        }
        Ok(Self { text, lines, key: None, offsets: vec![0.0] })
    }

    /// The complete text, as copied and exported.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Number of display lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Whether every line has been measured at `width` and `surface`'s scale.
    pub fn measured(&self, width: f32, surface: PaintSpec) -> bool {
        self.key == Some(Self::key(width, surface)) && self.offsets.len() == self.lines.len() + 1
    }

    /// Measure at most `budget` further lines at `width`, restarting when the width or scale
    /// changed. Returns whether the whole text is measured.
    /// # Errors
    /// Preserves font and memory-admission failures.
    pub fn measure(
        &mut self,
        width: f32,
        surface: PaintSpec,
        resources: &mut SceneResources,
        lease: Option<&MemoryLease>,
        budget: usize,
    ) -> io::Result<bool> {
        let key = Self::key(width, surface);
        if self.key != Some(key) {
            self.key = Some(key);
            self.offsets.clear();
            self.offsets.push(0.0);
        }
        for _ in 0..budget {
            let index = self.offsets.len() - 1;
            let Some(line) = self.lines.get(index) else { break };
            let text = &self.text[line.start..line.end];
            let height = if text.is_empty() {
                Self::LINE_HEIGHT
            } else {
                let spec =
                    LabelPaintSpec { surface, layout: Self::layout(0.0, width, 8192.0, [0; 4]) };
                resources.measure(text, spec, lease)?.height.max(Self::LINE_HEIGHT)
            };
            let top = self.offsets[index];
            self.offsets.push(top + height);
        }
        Ok(self.offsets.len() == self.lines.len() + 1)
    }

    /// Number of display lines measured so far at the current width.
    pub fn line_count_measured(&self) -> usize {
        self.offsets.len() - 1
    }

    /// Height of everything measured so far.
    pub fn height(&self) -> f32 {
        self.offsets.last().copied().unwrap_or(0.0)
    }

    /// Measured lines intersecting the half-open band `[top, bottom)`: text, top and height.
    pub fn visible(&self, top: f32, bottom: f32) -> impl Iterator<Item = (&str, f32, f32)> {
        let measured = self.offsets.len().saturating_sub(1);
        let first =
            self.offsets[..measured].partition_point(|offset| *offset <= top).saturating_sub(1);
        (first..measured).map_while(move |index| {
            let y = self.offsets[index];
            (y < bottom).then(|| {
                let line = self.lines[index];
                (&self.text[line.start..line.end], y, self.offsets[index + 1] - y)
            })
        })
    }

    /// Typography of one details line at a logical origin.
    pub fn layout(y: f32, width: f32, height: f32, color: [u8; 4]) -> LabelLayout {
        LabelLayout {
            origin: [0.0, y],
            extent: [width, height.max(Self::LINE_HEIGHT)],
            clip: None,
            anchor: VerticalAnchor::Top,
            alignment: TextAlignment::Leading,
            metrics: TextMetrics::Natural,
            font_size: Self::FONT_SIZE,
            line_height: Self::LINE_HEIGHT,
            weight: 400,
            color,
        }
    }

    fn key(width: f32, surface: PaintSpec) -> (u32, u32) {
        (width.to_bits(), surface.scale.to_bits())
    }
}
