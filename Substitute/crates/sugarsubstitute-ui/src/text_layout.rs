// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Geometry and typography supplied by native surface composition owners.

use std::io;
use sugarsubstitute_contracts::configuration::Theme;

/// Immutable target identity in device pixels, with its logical-to-device scale.
#[derive(Clone, Copy)]
pub struct PaintSpec {
    /// Palette used by styled console text.
    pub theme: Theme,
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// Target width in device pixels.
    pub width: u32,
    /// Target height in device pixels.
    pub height: u32,
}

/// Interpret the label's logical vertical origin using actual shaped baselines.
#[derive(Clone, Copy)]
pub enum VerticalAnchor {
    /// The origin is the top of the shaping layout.
    Top,
    /// Align the first shaped baseline with the origin.
    FirstBaseline,
    /// Align the last shaped baseline with the origin.
    LastBaseline,
}

/// Horizontal alignment within the bounded shaping layout.
#[derive(Clone, Copy)]
pub enum TextAlignment {
    /// Align with the leading edge of the text direction.
    Leading,
    /// Center within the layout width.
    Center,
    /// Align with the trailing edge of the text direction.
    Trailing,
}

/// Native measurement and glyph placement within a logical label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextMetrics {
    /// Preserve GDI-compatible advances and device-grid placement on Windows.
    GdiCompatible,
    /// Preserve natural fractional advances and glyph origins with the specified
    /// uniform line height. Portable shaping uses natural metrics for both selections.
    Natural,
}

/// Shaped logical size before clipping, produced by the same layout used for painting.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextSize {
    /// Advance width, including trailing whitespace, in logical pixels.
    pub width: f32,
    /// Full text height in logical pixels, independent of the visible viewport.
    pub height: f32,
    /// Number of shaped lines; empty text occupies no lines.
    pub lines: u32,
}

/// A bounded label's logical layout, typography and clipping policy.
#[derive(Clone, Copy)]
pub struct LabelLayout {
    /// Logical origin; vertical interpretation follows `anchor`.
    pub origin: [f32; 2],
    /// Positive shaping width and height in logical pixels.
    pub extent: [f32; 2],
    /// Optional half-open logical clipping rectangle, independent of the layout.
    pub clip: Option<[f32; 4]>,
    /// Actual shaped baseline or layout-top placement.
    pub anchor: VerticalAnchor,
    /// Horizontal layout alignment.
    pub alignment: TextAlignment,
    /// Measurement and placement rule, independent of the surface's layout.
    pub metrics: TextMetrics,
    /// Font em size in logical pixels.
    pub font_size: f32,
    /// Uniform natural-layout line height in logical pixels. GDI-compatible Windows
    /// labels preserve their native font line spacing.
    pub line_height: f32,
    /// OpenType weight in the inclusive range 1 through 999.
    pub weight: u16,
    /// Unpremultiplied sRGB foreground.
    pub color: [u8; 4],
}

/// Couple a label with the exact destination admitted by its preparation owner.
#[derive(Clone, Copy)]
pub struct LabelPaintSpec {
    /// Device target and scale.
    pub surface: PaintSpec,
    /// Logical typography and geometry.
    pub layout: LabelLayout,
}

impl LabelPaintSpec {
    /// Reject invalid external geometry before shaping or destination mutation.
    /// # Errors
    /// Returns invalid input for nonfinite, excessive or reversed coordinates.
    pub fn validate(self, message: &str) -> io::Result<()> {
        super::memory::validate(
            message,
            self.surface.scale,
            self.surface.width,
            self.surface.height,
        )?;
        let l = self.layout;
        if l.origin.iter().any(|v| !v.is_finite() || v.abs() > 16_384.0)
            || l.extent.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v > 8192.0)
            || !l.font_size.is_finite()
            || !(6.0..=128.0).contains(&l.font_size)
            || !l.line_height.is_finite()
            || !(l.font_size..=256.0).contains(&l.line_height)
            || !(1..=999).contains(&l.weight)
            || l.clip.is_some_and(|c| {
                c.iter().any(|v| !v.is_finite() || v.abs() > 16_384.0) || c[2] < c[0] || c[3] < c[1]
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid bounded label geometry",
            ));
        }
        Ok(())
    }

    /// Clip native raster work to a half-open device rectangle.
    pub(super) fn device_clip(self) -> [i32; 4] {
        self.layout
            .clip
            .map_or([0, 0, self.surface.width as i32, self.surface.height as i32], |c| {
                c.map(|v| (v * self.surface.scale).round() as i32)
            })
    }
}
#[cfg(test)]
#[path = "text_layout_tests.rs"]
mod tests;
