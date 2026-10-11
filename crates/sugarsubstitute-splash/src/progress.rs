// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Paint source-compatible completion and activity without estimating startup work.

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap, Rect,
    SpreadMode, Stroke, Transform,
};

use crate::{Appearance, Theme, shapes::rounded};

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
pub(crate) fn paint(target: &mut Pixmap, appearance: Appearance, frame: ProgressFrame, scale: f32) {
    let expanded = appearance.expanded;
    let (x, y, width, height) =
        (24.0, if expanded { 356.0 } else { 530.0 }, 510.0, if expanded { 6.0 } else { 4.0 });
    let transform = Transform::from_scale(scale, scale);
    let filled = (width * frame.completed * 10_000.0).round() / 10_000.0;
    let mut paint = Paint { anti_alias: true, ..Paint::default() };
    let track_alpha =
        if expanded { if appearance.theme == Theme::Dark { 18 } else { 12 } } else { 155 };
    paint.set_color_rgba8(
        if appearance.theme == Theme::Dark { 255 } else { 0 },
        if appearance.theme == Theme::Dark { 255 } else { 0 },
        if appearance.theme == Theme::Dark { 255 } else { 0 },
        track_alpha,
    );
    if expanded {
        if let Some(path) = rounded(x, y, width, 12.0, 6.0)
            && let Some(rect) = Rect::from_xywh(x, y, width, height)
            && let Some(mut mask) = Mask::new(target.width(), target.height())
        {
            mask.fill_path(&PathBuilder::from_rect(rect), FillRule::Winding, true, transform);
            target.fill_path(&path, &paint, FillRule::Winding, transform, Some(&mask));
        }
    } else {
        let mut path = PathBuilder::new();
        path.move_to(x, y + 2.0);
        path.line_to(x + width, y + 2.0);
        if let Some(path) = path.finish() {
            target.stroke_path(
                &path,
                &paint,
                &Stroke { width: 1.0, ..Stroke::default() },
                transform,
                None,
            );
        }
    }
    paint.set_color_rgba8(
        if appearance.theme == Theme::Dark { 255 } else { 233 },
        if appearance.theme == Theme::Dark { 68 } else { 30 },
        if appearance.theme == Theme::Dark { 132 } else { 99 },
        255,
    );
    let completion_path = clip_path(expanded, x, y, filled, height);
    let completed_mask = console_mask(target, expanded, x, y, width, scale);
    if let Some(path) = &completion_path {
        target.fill_path(path, &paint, FillRule::Winding, transform, completed_mask.as_ref());
    }
    if let Some(phase) = frame.activity {
        let activity_width = if filled > 0.0 { filled } else { width };
        let band = (activity_width * 0.35).max(12.0);
        let center = -band + phase * (activity_width + 2.0 * band);
        if let Some(shader) = LinearGradient::new(
            tiny_skia::Point::from_xy(x + center - band, y),
            tiny_skia::Point::from_xy(x + center + band, y),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(255, 255, 255, 0)),
                GradientStop::new(0.5, Color::from_rgba8(255, 255, 255, 65)),
                GradientStop::new(1.0, Color::from_rgba8(255, 255, 255, 0)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) && let Some(path) = clip_path(expanded, x, y, activity_width, height)
        {
            paint.shader = shader;
            target.fill_path(&path, &paint, FillRule::Winding, transform, completed_mask.as_ref());
        }
    }
}

/// Use a flat cut at incomplete console fill and rounded ends for standalone fill.
fn clip_path(expanded: bool, x: f32, y: f32, width: f32, height: f32) -> Option<tiny_skia::Path> {
    if expanded {
        Rect::from_xywh(x, y, width, height).map(PathBuilder::from_rect)
    } else {
        rounded(x, y, width, height, 2.0)
    }
}

/// Restrict the top console strip to its rounded outer silhouette.
fn console_mask(
    target: &Pixmap,
    expanded: bool,
    x: f32,
    y: f32,
    width: f32,
    scale: f32,
) -> Option<Mask> {
    if !expanded {
        return None;
    }
    let mut mask = Mask::new(target.width(), target.height())?;
    let path = rounded(x, y, width, 12.0, 6.0)?;
    mask.fill_path(&path, FillRule::Winding, true, Transform::from_scale(scale, scale));
    Some(mask)
}

#[cfg(test)]
mod tests {
    use super::{ProgressFrame, paint};
    use crate::Appearance;
    use tiny_skia::Pixmap;

    /// Reject invalid producer fractions without converting them into artificial progress.
    #[test]
    fn progress_inputs_require_finite_bounded_units() {
        assert!(ProgressFrame { completed: 0.4, activity: Some(0.5) }.is_valid());
        assert!(!ProgressFrame { completed: f32::NAN, activity: None }.is_valid());
        assert!(!ProgressFrame { completed: 0.0, activity: Some(2.0) }.is_valid());
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
        );
        paint(
            &mut active,
            Appearance::demo(),
            ProgressFrame { completed: 0.4, activity: Some(0.5) },
            1.0,
        );
        assert_eq!(still.pixel(300, 358), active.pixel(300, 358));
        assert_ne!(still.pixel(100, 358), active.pixel(100, 358));
        assert_eq!(still.pixel(100, 358).map(|pixel| pixel.alpha()), Some(255));
        assert_eq!(active.pixel(300, 358).map(|pixel| pixel.alpha()), Some(18));
        Ok(())
    }
}
