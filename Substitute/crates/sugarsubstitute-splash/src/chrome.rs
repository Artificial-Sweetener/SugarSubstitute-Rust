// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Place shared native widgets in the splash's original layout.

use crate::{Appearance, Control, Theme};
use sugarsubstitute_ui::panel::OutputPanel;
#[cfg(test)]
use {
    sugarsubstitute_ui::bitmap::Bitmap,
    sugarsubstitute_ui::{panel::PanelCursor, shapes::AllocationError},
};

/// Resolve the splash's animated output placement without owning widget rendering.
pub(crate) fn panel(appearance: Appearance) -> OutputPanel {
    let bounds = appearance.feedback();
    OutputPanel {
        x: bounds.x as f32,
        y: bounds.y as f32,
        width: bounds.width as f32,
        height: bounds.height as f32,
        dark: appearance.theme == Theme::Dark,
    }
}

#[cfg(test)]
fn console(target: &mut Bitmap, theme: Theme, scale: f32) -> Result<(), AllocationError> {
    let mut cursor = PanelCursor::new(
        target.width(),
        target.height(),
        panel(Appearance { theme, ..Appearance::demo() }),
        scale,
    )?;
    while !cursor.step(target)? {}
    Ok(())
}

/// Place shared native caption widgets inside the splash's original hit rectangles.
pub(crate) fn caption_buttons(
    appearance: Appearance,
) -> [sugarsubstitute_ui::caption::CaptionButton; 2] {
    use sugarsubstitute_ui::caption::{CaptionAction, CaptionButton, CaptionState};
    [(Control::ConsoleToggle, 466.0), (Control::Close, 512.0)].map(|(control, x)| CaptionButton {
        x,
        y: 0.0,
        action: if control == Control::Close {
            CaptionAction::Close
        } else {
            CaptionAction::Auxiliary
        },
        dark: appearance.theme == Theme::Dark,
        state: CaptionState {
            pressed: appearance.pressed == Some(control),
            hovered: appearance.hovered == Some(control),
            selected: control == Control::ConsoleToggle && appearance.expanded,
        },
    })
}

#[cfg(test)]
fn controls(
    target: &mut Bitmap,
    appearance: Appearance,
    scale: f32,
) -> Result<(), AllocationError> {
    for button in caption_buttons(appearance) {
        button.paint(target, scale)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{console, controls};
    use crate::{Appearance, Control, Theme};
    use sugarsubstitute_ui::bitmap::Bitmap;

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
                    Bitmap::new(1116, 1116).ok_or("Cannot allocate widget-clip raster")?;
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
        let mut target = Bitmap::new(977, 56).ok_or("Cannot allocate caption raster")?;
        controls(&mut target, Appearance::demo(), 1.75)?;
        assert_eq!(target.pixel(815, 12).map(|pixel| pixel.alpha()), Some(0));
        assert_eq!(target.pixel(816, 12).map(|pixel| pixel.alpha()), Some(28));
        Ok(())
    }

    /// Preserve native fillRect coverage for colored caption states at fractional far edges.
    #[test]
    fn fractional_close_edge_retains_native_byte_color() -> Result<(), Box<dyn std::error::Error>> {
        for (scale, edge) in [(1.25, 697), (1.75, 976)] {
            for (pressed, expected) in [(false, [116, 8, 17, 127]), (true, [120, 56, 61, 127])] {
                let mut target = Bitmap::new(977, 56).ok_or("Cannot allocate caption raster")?;
                controls(
                    &mut target,
                    Appearance {
                        hovered: Some(Control::Close),
                        pressed: pressed.then_some(Control::Close),
                        ..Appearance::demo()
                    },
                    scale,
                )?;
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
            let mut target = Bitmap::new(558, 558).ok_or("Cannot allocate console raster")?;
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
            let mut target = Bitmap::new(1116, 1116).ok_or("Cannot allocate corner raster")?;
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
