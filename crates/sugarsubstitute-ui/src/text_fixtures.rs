// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fixed independent label fixtures for renderer characterization.

use crate::text::{LabelLayout, LabelPaintSpec, PaintSpec, TextAlignment, VerticalAnchor};
use sugarsubstitute_contracts::configuration::Theme;

/// Preserve the status label's platform shaping and placement in the splash composition.
pub(super) fn status(
    theme: Theme,
    scale: f32,
    width: u32,
    height: u32,
    native: bool,
) -> LabelPaintSpec {
    LabelPaintSpec {
        surface: PaintSpec { theme, scale, width, height },
        layout: LabelLayout {
            origin: [24.0, if native { 518.0 } else { 519.0 }],
            extent: [510.0, 36.0],
            clip: if native { Some([24.0, 486.0, 534.0, 530.0]) } else { None },
            anchor: if native {
                VerticalAnchor::LastBaseline
            } else {
                VerticalAnchor::FirstBaseline
            },
            alignment: TextAlignment::Leading,
            metrics: crate::text::TextMetrics::GdiCompatible,
            font_size: 14.0,
            line_height: 16.0,
            weight: 400,
            color: if theme == Theme::Dark { [255; 4] } else { [0, 0, 0, 255] },
        },
    }
}
