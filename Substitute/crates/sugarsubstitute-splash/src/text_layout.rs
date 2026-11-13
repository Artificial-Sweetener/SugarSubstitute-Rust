// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Splash-specific label and transcript placement.

use crate::Theme;
use sugarsubstitute_ui::text::{
    LabelLayout, LabelPaintSpec, PaintSpec, TextAlignment, VerticalAnchor,
    console::ConsolePaintSpec,
};

/// Preserve the status label's platform shaping and placement in the splash composition.
pub(crate) fn status(theme: Theme, scale: f32, width: u32, height: u32) -> LabelPaintSpec {
    LabelPaintSpec {
        surface: PaintSpec { theme, scale, width, height },
        layout: LabelLayout {
            origin: [24.0, if cfg!(windows) { 518.0 } else { 519.0 }],
            extent: [510.0, 36.0],
            clip: if cfg!(windows) { Some([24.0, 486.0, 534.0, 530.0]) } else { None },
            anchor: if cfg!(windows) {
                VerticalAnchor::LastBaseline
            } else {
                VerticalAnchor::FirstBaseline
            },
            alignment: TextAlignment::Leading,
            metrics: sugarsubstitute_ui::text::TextMetrics::GdiCompatible,
            font_size: 14.0,
            line_height: 16.0,
            weight: 400,
            color: if theme == Theme::Dark { [255; 4] } else { [0, 0, 0, 255] },
        },
    }
}

/// Place one of the splash's bounded tail rows without giving the widget owner tail policy.
pub(crate) fn console(
    theme: Theme,
    scale: f32,
    width: u32,
    height: u32,
    row: usize,
    rows: usize,
) -> std::io::Result<ConsolePaintSpec> {
    if rows == 0 || rows > 10 || row >= rows {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid splash console row",
        ));
    }
    let spec = ConsolePaintSpec {
        surface: PaintSpec { theme, scale, width, height },
        origin: [30.0, 528.0 - (rows - row) as f32 * 16.0],
        extent: [498.0, 16.0],
        bounds: [30.0, 362.0, 528.0, 528.0],
        font_size: 14.0,
    };
    spec.validate()?;
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tail policy validates indices before computing unsigned row differences.
    #[test]
    fn console_row_placement_rejects_invalid_indices_and_keeps_the_tail_at_the_bottom()
    -> Result<(), Box<dyn std::error::Error>> {
        for (row, rows) in [(0, 0), (0, 11), (1, 1), (usize::MAX, 10)] {
            assert!(console(Theme::Dark, 1.0, 558, 558, row, rows).is_err());
        }
        assert_eq!(console(Theme::Dark, 1.0, 558, 558, 9, 10)?.top(), 512.0);
        assert_eq!(console(Theme::Dark, 1.0, 558, 558, 0, 10)?.top(), 368.0);
        Ok(())
    }
}
