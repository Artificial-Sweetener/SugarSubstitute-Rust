// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared progress widget; completion and activity are supplied by the application.

use crate::accent::AccentPalette;
use crate::bitmap::Bitmap;
use crate::shapes::{intersect, rounded};
use tiny_skia::{Paint, Rect, Transform};

#[path = "progress_render.rs"]
mod raster;
pub use raster::ProgressCursor;

/// Validated by the renderer before allocating or changing a destination.
#[derive(Clone, Copy, Debug)]
pub struct ProgressFrame {
    /// Observed completion in the inclusive range zero through one.
    pub completed: f32,
    /// Activity highlight phase; absence stops the highlight.
    pub activity: Option<f32>,
}

impl ProgressFrame {
    /// Reject non-finite fractions without inventing completion.
    pub fn is_valid(self) -> bool {
        self.completed.is_finite()
            && (0.0..=1.0).contains(&self.completed)
            && self.activity.is_none_or(|phase| phase.is_finite() && (0.0..=1.0).contains(&phase))
    }
}

/// Supported Fluent progress treatments share one renderer and coverage algorithm.
#[derive(Clone, Copy, Debug)]
pub enum ProgressAppearance {
    /// One-logical-pixel track beneath a rounded completion bar.
    Hairline {
        /// Select foreground and accent for a dark background.
        dark: bool,
    },
    /// Completion forms the top edge of a rounded output panel.
    Panel {
        /// Select foreground and accent for a dark background.
        dark: bool,
    },
    /// Rounded installer journey and operation progress.
    Wizard {
        /// Select completion and track colors for the resolved surface mode.
        dark: bool,
    },
}

/// Logical widget coordinates; the native host supplies the device scale separately.
#[derive(Clone, Copy, Debug)]
pub struct ProgressBar {
    /// Left edge in logical pixels.
    pub x: f32,
    /// Top edge in logical pixels.
    pub y: f32,
    /// Full track width, independent of completion.
    pub width: f32,
    /// Visible bar height.
    pub height: f32,
    /// Shared theme treatment.
    pub appearance: ProgressAppearance,
}

impl ProgressBar {
    /// Reject invalid geometry before raster allocation.
    pub(super) fn valid(self, scale: f32) -> bool {
        [self.x, self.y, self.width, self.height, scale].iter().all(|v| v.is_finite())
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && (0.0..=4.0).contains(&scale)
            && scale > 0.0
            && (self.x + self.width) * scale <= 4096.0
            && (self.y + self.height) * scale <= 4096.0
    }

    /// Form the complete silhouette before applying the completion cut.
    fn clip(self, width: f32, scale: f32) -> Option<tiny_skia::Path> {
        if matches!(self.appearance, ProgressAppearance::Panel { .. }) {
            if width <= 0.0 {
                return None;
            }
            let path = rounded(self.x, self.y, self.width, 12.0, 6.0, scale)?;
            intersect(&path, (self.x + width) * scale, (self.y + self.height) * scale)
        } else {
            let radius = if matches!(self.appearance, ProgressAppearance::Wizard { .. }) {
                3.0
            } else {
                2.0
            };
            rounded(self.x, self.y, width, self.height, radius, scale)
        }
    }

    /// Resolve completion and track colors here rather than in each surface's draw code.
    /// Completion always uses the surface's accent; the neutral track depends only on theme.
    fn colors(self, accent: AccentPalette) -> ([u8; 4], [u8; 4]) {
        match self.appearance {
            ProgressAppearance::Hairline { dark } | ProgressAppearance::Panel { dark } => {
                (accent.progress(dark), if dark { [255, 255, 255, 18] } else { [0, 0, 0, 12] })
            }
            ProgressAppearance::Wizard { dark } => {
                (accent.progress(dark), if dark { [255, 255, 255, 56] } else { [0, 0, 0, 56] })
            }
        }
    }
}

/// Integrate the reference's horizontal stroke without raster pen snapping.
fn standalone_track(target: &mut Bitmap, bar: ProgressBar, foreground: u8, scale: f32) {
    let left = (bar.x * scale).round();
    let right = ((bar.x + bar.width) * scale).round();
    let center = (bar.y + bar.height / 2.0) * scale;
    let top = center - scale / 2.0;
    let bottom = center + scale / 2.0;
    let mut paint = Paint { anti_alias: false, ..Paint::default() };
    for row in top.floor().max(0.0) as u32..bottom.ceil().min(target.height() as f32) as u32 {
        let coverage = ((row as f32 + 1.0).min(bottom) - (row as f32).max(top)).clamp(0.0, 1.0);
        // Qt's half-covered integer-center pen splits odd alpha between its two rows.
        let rounding = u16::from(coverage == 0.5 && row as f32 >= center);
        let coverage = (coverage * 255.0).floor() as u16 + rounding;
        let product = 155 * coverage;
        let alpha = ((product + (product >> 8) + 128) >> 8) as u8;
        paint.set_color_rgba8(foreground, foreground, foreground, alpha);
        if let Some(rect) = Rect::from_xywh(left, row as f32, right - left, 1.0) {
            target.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
}

#[cfg(test)]
mod tests;
