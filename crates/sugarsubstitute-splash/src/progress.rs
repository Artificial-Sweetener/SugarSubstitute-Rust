// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Paint source-compatible completion and activity without estimating startup work.

use tiny_skia::{Mask, Paint, PathBuilder, Pixmap, Rect, Transform};

use crate::{
    Appearance, Theme,
    shapes::{
        AllocationError, byte_product, coverage, intersect, paint as paint_silhouette, rounded,
    },
};

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
pub(crate) fn paint(
    target: &mut Pixmap,
    appearance: Appearance,
    frame: ProgressFrame,
    scale: f32,
) -> Result<(), AllocationError> {
    let expanded = appearance.expanded;
    let (x, y, width, height) =
        (24.0, if expanded { 356.0 } else { 530.0 }, 510.0, if expanded { 6.0 } else { 4.0 });
    let completed_width = width * frame.completed;
    let filled = if expanded { completed_width } else { completed_width.floor() };
    let foreground = if appearance.theme == Theme::Dark { 255 } else { 0 };
    let outer = console_mask(target, expanded, x, y, width, scale)?;
    if expanded {
        let alpha = if appearance.theme == Theme::Dark { 18 } else { 12 };
        if let Some(path) = clip_path(true, x, y, width, height, scale) {
            paint_silhouette(
                target,
                &path,
                [foreground, foreground, foreground, alpha],
                Some(&outer),
            )?;
        }
    } else {
        standalone_track(target, foreground, scale);
    }
    let color =
        if appearance.theme == Theme::Dark { [255, 68, 132, 255] } else { [233, 30, 99, 255] };
    if let Some(path) = clip_path(expanded, x, y, filled, height, scale) {
        paint_silhouette(target, &path, color, Some(&outer))?;
    }
    if let Some(phase) = frame.activity {
        let activity_width = if filled > 0.0 { filled } else { width };
        activity_sweep(
            target,
            Some(&outer),
            (x, y, activity_width, height),
            phase,
            scale,
            expanded,
        )?;
    }
    Ok(())
}

/// Match the reference sweep's 1024 samples, 256 interpolation weights and byte composition.
fn activity_sweep(
    target: &mut Pixmap,
    console: Option<&Mask>,
    geometry: (f32, f32, f32, f32),
    phase: f32,
    scale: f32,
    expanded: bool,
) -> Result<(), AllocationError> {
    let (x, y, width, height) = geometry;
    let Some(path) = clip_path(expanded, x, y, width, height, scale) else { return Ok(()) };
    let mut clip = coverage(&path, target.width(), target.height())?;
    if let Some(console) = console {
        for (coverage, silhouette) in clip.data_mut().iter_mut().zip(console.data()) {
            let product = u16::from(*coverage) * u16::from(*silhouette);
            *coverage = ((product + (product >> 8) + 128) >> 8) as u8;
        }
    }
    let band = (f64::from(width) * 0.35).max(12.0);
    let center = f64::from(x) - band + f64::from(phase) * (f64::from(width) + 2.0 * band);
    let left = (x * scale).floor().max(0.0) as u32;
    let origin = (((f64::from(left) + 0.5) / f64::from(scale) - center + band) / (2.0 * band)
        * 1023.0
        * 256.0) as i64;
    let increment = (1023.0 * 256.0 / (2.0 * band * f64::from(scale))) as i64;
    let top = (y * scale).floor().max(0.0) as u32;
    let bottom = ((y + height) * scale).ceil().min(target.height() as f32) as u32;
    for column in left..((x + width) * scale).ceil().min(target.width() as f32) as u32 {
        let position = origin + i64::from(column - left) * increment;
        let table_index = ((position + 128) >> 8).clamp(0, 1023) as f64;
        let table_position = (table_index + 0.5) / 1024.0;
        let weight = (512.0 * table_position.min(1.0 - table_position)).round() as u32;
        let alpha16 = 65 * 257 * weight / 256;
        let alpha = ((alpha16 + 128) / 257) as u8;
        for row in top..bottom {
            let index = (row * target.width() + column) as usize;
            // The source clips the sweep, then paints the widget rectangle as a second
            // antialiased shape. Fractional outer rows receive both coverage factors.
            let horizontal = ((column as f32 + 1.0).min((x + 510.0) * scale)
                - (column as f32).max(x * scale))
            .clamp(0.0, 1.0);
            let vertical = ((row as f32 + 1.0).min((y + height) * scale)
                - (row as f32).max(y * scale))
            .clamp(0.0, 1.0);
            let fill_coverage = (horizontal * vertical * 255.0).floor() as u8;
            let coverage = byte_product(alpha, byte_product(clip.data()[index], fill_coverage));
            let pixels = target.data_mut();
            for channel in &mut pixels[index * 4..index * 4 + 4] {
                *channel = coverage + byte_product(*channel, 255 - coverage);
            }
        }
    }
    Ok(())
}

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

/// Retain the bar child widget's device clip and the console's rounded outer silhouette.
fn console_mask(
    target: &Pixmap,
    expanded: bool,
    x: f32,
    y: f32,
    width: f32,
    scale: f32,
) -> Result<Mask, AllocationError> {
    let height = if expanded { 6.0 } else { 4.0 };
    let path = {
        let left = (x * scale).round();
        let top = (y * scale).round();
        PathBuilder::from_rect(
            Rect::from_xywh(
                left,
                top,
                ((x + width) * scale).round() - left,
                ((y + height) * scale).round() - top,
            )
            .ok_or(AllocationError::InvalidDimensions)?,
        )
    };
    let mut mask = coverage(&path, target.width(), target.height())?;
    let top = (y * scale).round().clamp(0.0, target.height() as f32) as usize;
    let bottom = ((y + height) * scale).round().clamp(0.0, target.height() as f32) as usize;
    let stride = target.width() as usize;
    mask.data_mut()[..top * stride].fill(0);
    mask.data_mut()[bottom * stride..].fill(0);
    Ok(mask)
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
            let appearance = Appearance { expanded: false, ..Appearance::demo() };
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
                Appearance { expanded: false, ..Appearance::demo() },
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
