// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Paint the original blank terminal silhouette and native-sized caption affordances.

use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::{Appearance, Control, Theme, shapes::rounded};

/// Retain the terminal's translucent fill and one-pixel theme borders.
pub(crate) fn console(target: &mut Pixmap, theme: Theme, scale: f32) {
    let mut paint = Paint { anti_alias: true, ..Paint::default() };
    let transform = Transform::from_scale(scale, scale);
    if let Some(path) = rounded(24.5, 356.5, 509.0, 177.0, 5.5) {
        match theme {
            Theme::Dark => paint.set_color_rgba8(8, 10, 12, 247),
            Theme::Light => paint.set_color_rgba8(252, 253, 255, 244),
        }
        target.fill_path(&path, &paint, FillRule::Winding, transform, None);
        let rgb = if theme == Theme::Dark { 255 } else { 0 };
        paint.set_color_rgba8(rgb, rgb, rgb, if theme == Theme::Dark { 56 } else { 41 });
        let mut border_mask = Mask::new(target.width(), target.height());
        if let Some(mask) = &mut border_mask
            && let Some(rect) = Rect::from_xywh(24.0, 356.0, 510.0, 177.0)
        {
            mask.fill_path(&PathBuilder::from_rect(rect), FillRule::Winding, false, transform);
        }
        target.stroke_path(
            &path,
            &paint,
            &Stroke { width: 1.0, ..Stroke::default() },
            transform,
            border_mask.as_ref(),
        );
        let mut bottom = PathBuilder::new();
        bottom.move_to(30.0, 533.5);
        bottom.line_to(528.0, 533.5);
        if let Some(path) = bottom.finish() {
            paint.set_color_rgba8(rgb, rgb, rgb, if theme == Theme::Dark { 112 } else { 61 });
            target.stroke_path(
                &path,
                &paint,
                &Stroke { width: 1.0, ..Stroke::default() },
                transform,
                None,
            );
        }
    }
}

/// Paint disclosure and close backgrounds inside their original logical hit rectangles.
pub(crate) fn controls(target: &mut Pixmap, appearance: Appearance, scale: f32) {
    let foreground = if appearance.theme == Theme::Dark { 255 } else { 0 };
    let mut paint = Paint { anti_alias: true, ..Paint::default() };
    for (control, x) in [(Control::ConsoleToggle, 466.0), (Control::Close, 512.0)] {
        let pressed = appearance.pressed == Some(control);
        let hovered = appearance.hovered == Some(control);
        if control == Control::Close && (pressed || hovered) {
            if pressed {
                paint.set_color_rgba8(241, 112, 122, 255);
            } else {
                paint.set_color_rgba8(232, 17, 35, 255);
            }
        } else {
            let opacity = if pressed {
                51
            } else if hovered {
                26
            } else if control == Control::ConsoleToggle && appearance.expanded {
                if appearance.theme == Theme::Dark { 28 } else { 18 }
            } else {
                0
            };
            paint.set_color_rgba8(foreground, foreground, foreground, opacity);
        }
        // Qt clips child widgets at their rounded device origin, retaining the fractional
        // painted far edge. A global rectangle otherwise leaks a half-alpha pixel leftward.
        let left = (x * scale).round();
        if let Some(rect) = Rect::from_xywh(left, 0.0, (x + 46.0) * scale - left, 32.0 * scale) {
            target.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::controls;
    use crate::Appearance;
    use tiny_skia::Pixmap;

    /// Keep checked caption fill inside its observed fractional-DPI child widget clip.
    #[test]
    fn fractional_caption_origin_does_not_leak_leftward() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut target = Pixmap::new(977, 56).ok_or("Cannot allocate caption raster")?;
        controls(&mut target, Appearance::demo(), 1.75);
        assert_eq!(target.pixel(815, 12).map(|pixel| pixel.alpha()), Some(0));
        assert_eq!(target.pixel(816, 12).map(|pixel| pixel.alpha()), Some(28));
        Ok(())
    }
}
