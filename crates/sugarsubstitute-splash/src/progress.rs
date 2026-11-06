// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the shared progress widget to the splash's animated panel layout.

use crate::{Appearance, Theme};
use sugarsubstitute_ui::progress::{ProgressAppearance, ProgressBar};

/// The splash owns positions; the UI owner owns shapes, colors and painting.
pub(crate) fn bar(appearance: Appearance) -> ProgressBar {
    let dark = appearance.theme == Theme::Dark;
    ProgressBar {
        x: 24.0,
        y: appearance.bar_y() as f32,
        width: 510.0,
        height: 4.0 + 2.0 * appearance.reveal as f32,
        appearance: if appearance.reveal > 0.0 {
            ProgressAppearance::Panel { dark }
        } else {
            ProgressAppearance::Hairline { dark }
        },
    }
}

/// Run production widget preparation synchronously only in layout qualification.
#[cfg(test)]
fn paint(
    target: &mut sugarsubstitute_ui::bitmap::Bitmap,
    appearance: Appearance,
    frame: sugarsubstitute_ui::progress::ProgressFrame,
    scale: f32,
) -> Result<(), sugarsubstitute_ui::shapes::AllocationError> {
    let mut cursor = sugarsubstitute_ui::progress::ProgressCursor::new(
        target.width(),
        target.height(),
        bar(appearance),
        frame,
        appearance.accent,
        scale,
    )?;
    while !cursor.step(target)? {}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::paint;
    use crate::Appearance;
    use sugarsubstitute_ui::bitmap::Bitmap;
    use sugarsubstitute_ui::progress::ProgressFrame;

    /// Keep activity inside the producer's completed region without altering completion.
    #[test]
    fn activity_highlights_work_without_advancing_completion()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut still = Bitmap::new(558, 558).ok_or("Cannot allocate qualification frame")?;
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
            let mut target = Bitmap::new(1116, 1116).ok_or("Cannot allocate progress raster")?;
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
        let mut target = Bitmap::new(558, 558).ok_or("Cannot allocate progress raster")?;
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
            let mut target = Bitmap::new(1116, 1116).ok_or("Cannot allocate corner raster")?;
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
