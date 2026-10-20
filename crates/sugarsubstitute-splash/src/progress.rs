// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Paint source-compatible completion and activity without estimating startup work.

use tiny_skia::{Paint, Pixmap, Rect, Transform};

use crate::shapes::{intersect, rounded};
#[cfg(test)]
use crate::{Appearance, shapes::AllocationError};

/// Validate externally supplied completion and activity presentation fractions.
#[derive(Clone, Copy, Debug)]
pub struct ProgressFrame {
    /// Producer-owned completion from zero through one.
    pub completed: f32,
    /// Optional activity sweep phase; absence means the sweep is stopped.
    pub activity: Option<f32>,
}

impl ProgressFrame {
    /// Reject non-finite or out-of-range presentation input before drawing.
    pub fn is_valid(self) -> bool {
        self.completed.is_finite()
            && (0.0..=1.0).contains(&self.completed)
            && self.activity.is_none_or(|phase| phase.is_finite() && (0.0..=1.0).contains(&phase))
    }
}

/// Paint the bar in its fixed logical position and source console-facing silhouette.
///
/// Return the underlying cause when a required physical coverage buffer cannot be allocated.
#[cfg(test)]
pub(crate) fn paint(
    target: &mut Pixmap,
    appearance: Appearance,
    frame: ProgressFrame,
    scale: f32,
) -> Result<(), AllocationError> {
    let mut cursor =
        ProgressCursor::new(target.width(), target.height(), appearance, frame, scale)?;
    while !cursor.step(target)? {}
    Ok(())
}
#[path = "progress_render.rs"]
mod raster;
pub(crate) use raster::ProgressCursor;

/// Integrate the reference's horizontal one-logical-pixel stroke without raster pen snapping.
fn standalone_track(target: &mut Pixmap, foreground: u8, scale: f32) {
    let left = (24.0 * scale).round();
    let right = (534.0 * scale).round();
    let center = 532.0 * scale;
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

/// Use a flat cut at incomplete console fill and rounded ends for standalone fill.
fn clip_path(
    expanded: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scale: f32,
) -> Option<tiny_skia::Path> {
    if expanded {
        if width <= 0.0 {
            return None;
        }
        let path = rounded(x, y, 510.0, 12.0, 6.0, scale)?;
        intersect(&path, (x + width) * scale, (y + height) * scale)
    } else {
        rounded(x, y, width, height, 2.0, scale)
    }
}

#[cfg(test)]
mod tests {
    use super::{ProgressFrame, clip_path, paint};
    use crate::Appearance;
    use tiny_skia::Pixmap;

    /// Reject invalid producer fractions without converting them into artificial progress.
    #[test]
    fn progress_inputs_require_finite_bounded_units() {
        assert!(ProgressFrame { completed: 0.4, activity: Some(0.5) }.is_valid());
        assert!(!ProgressFrame { completed: f32::NAN, activity: None }.is_valid());
        assert!(!ProgressFrame { completed: 0.0, activity: Some(2.0) }.is_valid());
    }

    /// Retain the actual native intersected silhouette's fractional bottom-corner area.
    #[test]
    fn expanded_clip_intersects_before_coverage() -> Result<(), Box<dyn std::error::Error>> {
        for (scale, x, y, expected) in
            [(1.25, 667, 452, 61), (1.75, 934, 633, 61), (2.0, 1066, 718, 156)]
        {
            let path =
                clip_path(true, 24.0, 356.0, 510.0, 6.0, scale).ok_or("Missing expanded clip")?;
            let mask = crate::shapes::coverage(&path, 1116, 1116)?;
            assert!(
                mask.data()[y * 1116 + x].abs_diff(expected) <= 1,
                "actual {} expected {expected}",
                mask.data()[y * 1116 + x]
            );
            if scale == 2.0 {
                assert_eq!(mask.data()[y * 1116 + x], expected);
            }
        }
        Ok(())
    }

    /// Keep activity inside the producer's completed region without altering completion.
    #[test]
    fn activity_highlights_work_without_advancing_completion()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut still = Pixmap::new(558, 558).ok_or("Cannot allocate qualification frame")?;
        let mut active = still.clone();
        paint(
            &mut still,
            Appearance::demo(),
            ProgressFrame { completed: 0.4, activity: None },
            1.0,
        )?;
        paint(
            &mut active,
            Appearance::demo(),
            ProgressFrame { completed: 0.4, activity: Some(0.5) },
            1.0,
        )?;
        assert_eq!(still.pixel(300, 358), active.pixel(300, 358));
        assert_ne!(still.pixel(100, 358), active.pixel(100, 358));
        assert_eq!(still.pixel(100, 358).map(|pixel| pixel.alpha()), Some(255));
        assert_eq!(active.pixel(300, 358).map(|pixel| pixel.alpha()), Some(18));
        Ok(())
    }

    /// Preserve the observed Qt pen's symmetric fractional coverage and split odd alpha.
    #[test]
    fn standalone_track_retains_reference_fractional_row_coverage()
    -> Result<(), Box<dyn std::error::Error>> {
        for (scale, rows) in [
            (1.0, [(531, 77), (532, 78)]),
            (1.25, [(664, 97), (665, 97)]),
            (1.5, [(797, 116), (798, 116)]),
            (1.75, [(930, 136), (931, 136)]),
            (2.0, [(1063, 155), (1064, 155)]),
        ] {
            let mut target = Pixmap::new(1116, 1116).ok_or("Cannot allocate progress raster")?;
            let appearance = Appearance { expanded: false, reveal: 0.0, ..Appearance::demo() };
            paint(
                &mut target,
                appearance,
                ProgressFrame { completed: 0.0, activity: None },
                scale,
            )?;
            for (row, alpha) in rows {
                assert_eq!(target.pixel(200, row).map(|pixel| pixel.alpha()), Some(alpha));
            }
        }
        Ok(())
    }

    /// Preserve the source gradient's quantization at both shoulders and its crest.
    #[test]
    fn activity_gradient_retains_reference_channel_values() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut target = Pixmap::new(558, 558).ok_or("Cannot allocate progress raster")?;
        paint(
            &mut target,
            Appearance::demo(),
            ProgressFrame { completed: 0.4, activity: Some(0.5) },
            1.0,
        )?;
        for (column, green, blue) in
            [(70, 78, 139), (110, 105, 157), (125, 115, 163), (130, 113, 161), (190, 72, 135)]
        {
            let pixel = target.pixel(column, 359).ok_or("Missing gradient pixel")?;
            assert_eq!(
                (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()),
                (255, green, blue, 255)
            );
        }
        Ok(())
    }
    /// Retain small corner coverage observed in the production painter at fractional DPI.
    #[test]
    fn completion_corners_retain_reference_area_coverage() -> Result<(), Box<dyn std::error::Error>>
    {
        for (scale, row, first, alphas) in [
            (1.0, 530, 24, [76, 232, 255]),
            (1.25, 662, 30, [0, 0, 0]),
            (1.5, 795, 36, [7, 139, 242]),
            (1.75, 927, 42, [0, 0, 0]),
        ] {
            let mut target = Pixmap::new(1116, 1116).ok_or("Cannot allocate corner raster")?;
            paint(
                &mut target,
                Appearance { expanded: false, reveal: 0.0, ..Appearance::demo() },
                ProgressFrame { completed: 0.4, activity: None },
                scale,
            )?;
            for (offset, alpha) in alphas.into_iter().enumerate() {
                assert_eq!(
                    target.pixel(first + offset as u32, row).map(|pixel| pixel.alpha()),
                    Some(alpha)
                );
            }
        }
        Ok(())
    }
}
