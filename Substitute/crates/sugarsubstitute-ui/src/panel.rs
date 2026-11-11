// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared rounded output panel with independently composited Fluent border edges.

use crate::shapes::{AllocationError, rounded};
use tiny_skia::{PathBuilder, Rect};

#[path = "panel_border.rs"]
mod border;
#[path = "panel_cursor.rs"]
mod cursor;
use cursor::ConsolePass;
pub use cursor::PanelCursor;

/// Logical placement and theme of a native output panel.
#[derive(Clone, Copy, Debug)]
pub struct OutputPanel {
    /// Left edge in logical client pixels.
    pub x: f32,
    /// Top edge in logical client pixels.
    pub y: f32,
    /// Width including its one-logical-pixel border.
    pub width: f32,
    /// Height including its border.
    pub height: f32,
    /// Select the dark-window fill and edge colors.
    pub dark: bool,
}

impl OutputPanel {
    /// Reject unsupported geometry before allocating or painting a coverage plane.
    fn validate(self, scale: f32) -> Result<(), AllocationError> {
        if ![self.x, self.y, self.width, self.height, scale].iter().all(|v| v.is_finite())
            || self.x < 0.0
            || self.y < 0.0
            || self.width <= 1.0
            || self.height <= 1.0
            || !(0.5..=4.0).contains(&scale)
            || (self.x + self.width) * scale > 4096.0
            || (self.y + self.height) * scale > 4096.0
        {
            return Err(AllocationError::InvalidDimensions);
        }
        Ok(())
    }
}

fn panel_passes(panel: OutputPanel, scale: f32) -> Result<Vec<ConsolePass>, AllocationError> {
    let OutputPanel { x, y: top, width, height, dark } = panel;
    let right = x + width;
    let lower = top + height;
    let corner = 6.0_f32.min(height / 2.0).min(width / 2.0);
    let left_center = x + corner;
    let right_center = right - corner;
    let upper_center = top + corner;
    let lower_center = lower - corner;
    let mut passes = Vec::with_capacity(13);
    let clip = Rect::from_ltrb(
        (x * scale).round(),
        (top * scale).round(),
        (right * scale).round(),
        (lower * scale).round(),
    )
    .ok_or(AllocationError::InvalidDimensions)?;
    if let Some(path) = rounded(x + 0.5, top + 0.5, width - 1.0, height - 1.0, corner - 0.5, scale)
    {
        let fill = if dark { [8, 10, 12, 247] } else { [252, 253, 255, 244] };
        passes.push(ConsolePass::Fill(path, fill));
    }
    let foreground = if dark { 255 } else { 0 };
    let side = if dark { 56 } else { 40 };
    let bottom = if dark { 112 } else { 61 };
    for (rectangle, corners, alpha) in [
        (
            (left_center, lower - 1.0, right_center - left_center, 1.0),
            [
                ((left_center, lower_center), 2, false, false),
                ((right_center, lower_center), 1, true, true),
            ],
            bottom,
        ),
        (
            (right - 1.0, upper_center, 1.0, lower_center - upper_center),
            [
                ((right_center, upper_center), 0, true, false),
                ((right_center, lower_center), 1, false, true),
            ],
            side,
        ),
        (
            (x, upper_center, 1.0, lower_center - upper_center),
            [
                ((left_center, upper_center), 3, false, true),
                ((left_center, lower_center), 2, true, true),
            ],
            side,
        ),
        (
            (left_center, top, right_center - left_center, 1.0),
            [
                ((left_center, upper_center), 3, true, false),
                ((right_center, upper_center), 0, false, true),
            ],
            side,
        ),
    ] {
        let color = [foreground, foreground, foreground, alpha];
        let (x, y, width, height) = rectangle;
        if let Some(rect) = Rect::from_xywh(x * scale, y * scale, width * scale, height * scale) {
            passes.push(ConsolePass::Fill(PathBuilder::from_rect(rect), color));
        }
        for (center, quadrant, second, reverse) in corners {
            passes.push(ConsolePass::Arc(
                arc_points(
                    (f64::from(center.0), f64::from(center.1)),
                    (quadrant, second, reverse),
                    f64::from(corner - 0.5),
                ),
                color,
                clip,
            ));
        }
    }
    Ok(passes)
}

/// Compute one independently capped half-corner in the original CSS order.
fn arc_points(center: (f64, f64), arc: (u8, bool, bool), radius: f64) -> [[f64; 2]; 4] {
    let (quadrant, second, reverse) = arc;
    let curve = radius * 0.552_284_749_8;
    let midpoint = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let a = [0.0, -radius];
    let b = [curve, -radius];
    let c = [radius, -curve];
    let d = [radius, 0.0];
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    let cd = midpoint(c, d);
    let abc = midpoint(ab, bc);
    let bcd = midpoint(bc, cd);
    let middle = midpoint(abc, bcd);
    let mut points = if second { [middle, bcd, cd, d] } else { [a, ab, abc, middle] };
    if reverse {
        points.reverse();
    }
    points.map(|mut point| {
        for _ in 0..quadrant {
            point = [-point[1], point[0]];
        }
        [point[0] + center.0, point[1] + center.1]
    })
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
