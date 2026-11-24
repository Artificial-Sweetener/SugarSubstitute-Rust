// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Rasterize one caption glyph per step: parse, rasterize and tint, then composite.

use super::{CaptionIcon, CaptionIcons, IconError, preparation};
use sugarsubstitute_ui::{
    bitmap::Bitmap,
    vector::{VectorPixels, VectorViewport},
};
use tiny_skia::{PixmapPaint, Transform};

/// Capture glyph selection without parsing or rasterizing on the submitting thread.
pub(crate) struct CaptionCursor {
    kind: CaptionIcon,
    tint: [u8; 3],
    scale: f64,
    glyph: Option<VectorPixels>,
    painted: bool,
}
impl CaptionCursor {
    /// Select one glyph at a bounded device scale.
    pub(crate) fn new(scale: f64, kind: CaptionIcon, tint: [u8; 3]) -> Result<Self, IconError> {
        if !scale.is_finite() || !(0.5..=4.0).contains(&scale) {
            return Err(IconError::InvalidScale);
        }
        Ok(Self { kind, tint, scale, glyph: None, painted: false })
    }

    /// Rasterize and tint the glyph (at most 184 by 128 device pixels), or composite it.
    pub(crate) fn step(
        &mut self,
        cache: &mut CaptionIcons,
        target: &mut Bitmap,
    ) -> Result<bool, IconError> {
        if self.painted {
            return Ok(true);
        }
        let Some(glyph) = &self.glyph else {
            let [x, y, width, height] = self.kind.bounds();
            let asset = cache.asset(self.kind.shared())?;
            let mut glyph = asset
                .rasterize(VectorViewport { x, y, width, height, scale: self.scale as f32 }, None)
                .map_err(preparation)?;
            let [red, green, blue] = self.tint;
            glyph.recolor([red, green, blue, 255]);
            self.glyph = Some(glyph);
            return Ok(false);
        };
        let [x, y] = glyph.origin();
        target.draw_bitmap(
            x,
            y,
            glyph.pixels(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        self.painted = true;
        Ok(true)
    }
}
