// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Paint the original blank terminal silhouette and native-sized caption affordances.

use tiny_skia::{PathBuilder, Pixmap, Rect};

use crate::{
    Appearance, Control, Theme,
    shapes::{AllocationError, rounded},
};

#[path = "border_raster.rs"]
mod border_raster;

/// Retain the terminal's translucent fill and independently painted CSS border edges.
///
/// Return the underlying cause when a required physical coverage buffer cannot be allocated.
#[cfg(test)]
pub(crate) fn console(
    target: &mut Pixmap,
    theme: Theme,
    scale: f32,
) -> Result<(), AllocationError> {
    let mut cursor = ConsoleCursor::new(target.width(), target.height(), theme, scale)?;
    while !cursor.step(target)? {}
    Ok(())
}
#[path = "chrome_console.rs"]
mod console_cursor;
pub(crate) use console_cursor::ConsoleCursor;
use console_cursor::ConsolePass;

fn console_passes(theme: Theme, scale: f32) -> Result<Vec<ConsolePass>, AllocationError> {
    let mut passes = Vec::with_capacity(13);
    let clip = Rect::from_ltrb(
        (24.0 * scale).round(),
        (356.0 * scale).round(),
        (534.0 * scale).round(),
        (534.0 * scale).round(),
    )
    .ok_or(AllocationError::InvalidDimensions)?;
    if let Some(path) = rounded(24.5, 356.5, 509.0, 177.0, 5.5, scale) {
        let fill = match theme {
            Theme::Dark => [8, 10, 12, 247],
            Theme::Light => [252, 253, 255, 244],
        };
        passes.push(ConsolePass::Fill(path, fill));
    }
    let foreground = if theme == Theme::Dark { 255 } else { 0 };
    let side = if theme == Theme::Dark { 56 } else { 40 };
    let bottom = if theme == Theme::Dark { 112 } else { 61 };
    for (rectangle, corners, alpha) in [
        (
            (30.0, 533.0, 498.0, 1.0),
            [((30.0, 528.0), 2, false, false), ((528.0, 528.0), 1, true, true)],
            bottom,
        ),
        (
            (533.0, 362.0, 1.0, 166.0),
            [((528.0, 362.0), 0, true, false), ((528.0, 528.0), 1, false, true)],
            side,
        ),
        (
            (24.0, 362.0, 1.0, 166.0),
            [((30.0, 362.0), 3, false, true), ((30.0, 528.0), 2, true, true)],
            side,
        ),
        (
            (30.0, 356.0, 498.0, 1.0),
            [((30.0, 362.0), 3, true, false), ((528.0, 362.0), 0, false, true)],
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
                arc_points(center, (quadrant, second, reverse)),
                color,
                clip,
            ));
        }
    }
    Ok(passes)
}

/// Compute one independently capped half-corner in the original CSS order.
fn arc_points(center: (f64, f64), arc: (u8, bool, bool)) -> [[f64; 2]; 4] {
    let (quadrant, second, reverse) = arc;
    let radius = 5.5;
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
/// Paint disclosure and close backgrounds inside their original logical hit rectangles.
#[cfg(test)]
pub(crate) fn controls(target: &mut Pixmap, appearance: Appearance, scale: f32) {
    let mut cursor = ControlsCursor::new(appearance, scale);
    while !cursor.step(target) {}
}
/// Retain caption background order while painting only one bounded button per slice.
pub(crate) struct ControlsCursor {
    appearance: Appearance,
    scale: f32,
    index: usize,
}
impl ControlsCursor {
    /// Capture two fixed hit rectangles without touching destination pixels.
    pub(crate) fn new(appearance: Appearance, scale: f32) -> Self {
        Self { appearance, scale, index: 0 }
    }
    /// Paint at most one button, bounded by 23680 physical pixels at scale<=4.
    pub(crate) fn step(&mut self, target: &mut Pixmap) -> bool {
        let Some((control, x)) =
            [(Control::ConsoleToggle, 466.0), (Control::Close, 512.0)].get(self.index).copied()
        else {
            return true;
        };
        self.index += 1;
        let appearance = self.appearance;
        let scale = self.scale;
        let foreground = if appearance.theme == Theme::Dark { 255 } else { 0 };
        let pressed = appearance.pressed == Some(control);
        let hovered = appearance.hovered == Some(control);
        let color = if control == Control::Close && (pressed || hovered) {
            if pressed { [241, 112, 122, 255] } else { [232, 17, 35, 255] }
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
            [foreground, foreground, foreground, opacity]
        };
        // Qt clips child widgets at their rounded device origin, retaining the fractional
        // painted far edge. A global rectangle otherwise leaks a half-alpha pixel leftward.
        let left = (x * scale).round();
        let right = (x + 46.0) * scale;
        let bottom = 32.0 * scale;
        for row in 0..bottom.ceil().min(target.height() as f32) as u32 {
            for column in left as u32..right.ceil().min(target.width() as f32) as u32 {
                let horizontal = (right - column as f32).clamp(0.0, 1.0);
                let vertical = (bottom - row as f32).clamp(0.0, 1.0);
                let coverage = (horizontal * vertical * 255.0).floor() as u8;
                border_raster::pixel(target, i64::from(column), i64::from(row), color, coverage);
            }
        }
        self.index == 2
    }
}

#[cfg(test)]
mod tests {
    use super::{console, controls};
    use crate::{Appearance, Control, Theme};
    use tiny_skia::Pixmap;

    /// Retain the QWidget's rectangular paint clip even when a cosmetic cap extends outward.
    #[test]
    fn console_border_preserves_pixels_outside_its_widget() -> Result<(), Box<dyn std::error::Error>>
    {
        for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
            let (left, top, right, bottom) = (
                (24.0 * scale).round() as u32,
                (356.0 * scale).round() as u32,
                (534.0 * scale).round() as u32,
                (534.0 * scale).round() as u32,
            );
            for theme in [Theme::Dark, Theme::Light] {
                let mut target =
                    Pixmap::new(1116, 1116).ok_or("Cannot allocate widget-clip raster")?;
                target.fill(tiny_skia::Color::from_rgba8(34, 51, 68, 255));
                console(&mut target, theme, scale)?;
                assert_ne!(target.pixel(left + 12, top + 12).map(|pixel| pixel.red()), Some(34));
                for y in 0..target.height() {
                    for x in 0..target.width() {
                        if x < left || x >= right || y < top || y >= bottom {
                            let pixel = target.pixel(x, y).ok_or("Missing widget-clip pixel")?;
                            assert_eq!(
                                [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
                                [34, 51, 68, 255],
                                "{theme:?} at {scale}: ({x}, {y})"
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

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

    /// Preserve native fillRect coverage for colored caption states at fractional far edges.
    #[test]
    fn fractional_close_edge_retains_native_byte_color() -> Result<(), Box<dyn std::error::Error>> {
        for (scale, edge) in [(1.25, 697), (1.75, 976)] {
            for (pressed, expected) in [(false, [116, 8, 17, 127]), (true, [120, 56, 61, 127])] {
                let mut target = Pixmap::new(977, 56).ok_or("Cannot allocate caption raster")?;
                controls(
                    &mut target,
                    Appearance {
                        hovered: Some(Control::Close),
                        pressed: pressed.then_some(Control::Close),
                        ..Appearance::demo()
                    },
                    scale,
                );
                let pixel = target.pixel(edge, 0).ok_or("Missing caption edge pixel")?;
                assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], expected);
            }
        }
        Ok(())
    }
    /// Preserve separately painted translucent CSS fill and stronger bottom border.
    #[test]
    fn console_straight_edges_retain_reference_byte_composition()
    -> Result<(), Box<dyn std::error::Error>> {
        for (theme, left, bottom) in [
            (Theme::Dark, (59, 60, 61, 153), (114, 115, 115, 182)),
            (Theme::Light, (102, 102, 103, 143), (92, 92, 93, 154)),
        ] {
            let mut target = Pixmap::new(558, 558).ok_or("Cannot allocate console raster")?;
            console(&mut target, theme, 1.0)?;
            for ((x, y), expected) in [((24, 500), left), ((100, 533), bottom)] {
                let pixel = target.pixel(x, y).ok_or("Missing console edge pixel")?;
                assert_eq!((pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()), expected);
            }
        }
        Ok(())
    }

    /// Match native Qt's independently capped CSS corners on both pen raster paths.
    #[test]
    fn console_corners_retain_native_pen_coverage() -> Result<(), Box<dyn std::error::Error>> {
        for (theme, scale, x, y, expected) in [
            (Theme::Dark, 1.0, 25, 531, [63, 63, 63, 96]),
            (Theme::Light, 1.0, 27, 533, [3, 3, 3, 42]),
            (Theme::Dark, 1.25, 34, 666, [95, 95, 95, 117]),
            (Theme::Light, 1.25, 31, 664, [0, 0, 0, 22]),
            (Theme::Dark, 1.5, 38, 796, [57, 58, 59, 221]),
            (Theme::Light, 1.75, 44, 929, [75, 75, 76, 116]),
            (Theme::Dark, 2.0, 54, 1066, [65, 65, 65, 65]),
        ] {
            let mut target = Pixmap::new(1116, 1116).ok_or("Cannot allocate corner raster")?;
            console(&mut target, theme, scale)?;
            let pixel = target.pixel(x, y).ok_or("Missing console corner pixel")?;
            let actual = [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()];
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert!(
                    actual.abs_diff(expected) <= 1,
                    "{theme:?} at {scale}: {actual} / {expected}"
                );
            }
        }
        Ok(())
    }
}
