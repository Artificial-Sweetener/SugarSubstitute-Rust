// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native label measurement and rasterization on the same bounded DirectWrite layout.
use super::*;
use crate::text::{TextAlignment, TextMetrics, TextSize};

impl FontOwner {
    /// Measure all wrapped lines on a preparation worker, before viewport clipping.
    /// Production callers supply admitted storage; `None` is for offline qualification.
    /// # Errors
    /// Preserves invalid input, allocation, admission and native shaping failures.
    pub fn measure(
        &mut self,
        message: &str,
        spec: LabelPaintSpec,
        lease: Option<&MemoryLease>,
    ) -> io::Result<TextSize> {
        spec.validate(message)?;
        let _claim = lease
            .map(|lease| lease.claim(METADATA_BYTES as u64))
            .transpose()
            .map_err(io::Error::other)?;
        if message.is_empty() {
            return Ok(TextSize::default());
        }
        let layout = self.layout(message, spec)?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        // SAFETY: This retained layout synchronously writes its documented metrics structure.
        unsafe { layout.GetMetrics(&mut metrics) }?;
        Ok(TextSize {
            width: metrics.widthIncludingTrailingWhitespace,
            height: metrics.height,
            lines: metrics.lineCount,
        })
    }

    /// Configure the sole layout used by measurement and drawing; no HWND or apartment is needed.
    pub(super) fn layout(
        &self,
        message: &str,
        spec: LabelPaintSpec,
    ) -> io::Result<IDWriteTextLayout> {
        spec.validate(message)?;
        let mut text = memory::reserve::<u16>(MAX_GLYPHS, METADATA_BYTES)?;
        text.extend(message.encode_utf16());
        // SAFETY: DirectWrite copies the bounded UTF-16 input and retains its font format. All
        // property buffers below are local, initialized and borrowed only by synchronous calls.
        unsafe {
            let layout = match spec.layout.metrics {
                TextMetrics::GdiCompatible => self.factory.CreateGdiCompatibleTextLayout(
                    &text,
                    &self.format,
                    spec.layout.extent[0],
                    spec.layout.extent[1],
                    1.0,
                    None,
                    false,
                )?,
                TextMetrics::Natural => self.factory.CreateTextLayout(
                    &text,
                    &self.format,
                    spec.layout.extent[0],
                    spec.layout.extent[1],
                )?,
            };
            let range = DWRITE_TEXT_RANGE { startPosition: 0, length: text.len() as u32 };
            if spec.layout.font_size != 14.0 {
                layout.SetFontSize(spec.layout.font_size, range)?;
            }
            if spec.layout.weight != 400 {
                layout.SetFontWeight(DWRITE_FONT_WEIGHT(i32::from(spec.layout.weight)), range)?;
            }
            layout.SetTextAlignment(match spec.layout.alignment {
                TextAlignment::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
                TextAlignment::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                TextAlignment::Trailing => DWRITE_TEXT_ALIGNMENT_TRAILING,
            })?;
            if spec.layout.metrics == TextMetrics::Natural && !text.is_empty() {
                let mut lines =
                    memory::reserve::<DWRITE_LINE_METRICS>(MAX_GLYPHS + 1, METADATA_BYTES)?;
                lines.resize(MAX_GLYPHS + 1, DWRITE_LINE_METRICS::default());
                let mut count = 0;
                layout.GetLineMetrics(Some(&mut lines), &mut count)?;
                let first = lines
                    .first()
                    .filter(|_| count > 0)
                    .ok_or_else(|| io::Error::other("Native text has no line metrics"))?;
                let baseline = first.baseline + (spec.layout.line_height - first.height) * 0.5;
                layout.SetLineSpacing(
                    DWRITE_LINE_SPACING_METHOD_UNIFORM,
                    spec.layout.line_height,
                    baseline,
                )?;
            }
            Ok(layout)
        }
    }
}
