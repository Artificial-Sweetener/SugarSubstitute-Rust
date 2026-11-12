// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show committed canvas pixels at changed geometry until a frame at that geometry replaces them.
//!
//! A resized camera keeps its centre, so the committed pixels stay centred in the current
//! viewport and are clipped to its clip. A fit-mode camera also changes zoom; the next prepared
//! frame corrects that within a frame or two instead of the surface going blank.

use super::{Canvas, missing};
use crate::{DesktopError, canvas::placement::CanvasPlacement};
use sugarsubstitute_image_ops::Extent;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

/// The visual offset and local clip that centre `extent` pixels in `shown`'s viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Centred {
    /// Physical client offset of the pixels' top-left corner.
    pub(super) offset: [i64; 2],
    /// Clip in the pixels' own coordinates; empty when none of them lie inside the clip.
    pub(super) clip: D2D_RECT_F,
}

/// Centre committed pixels of `extent` in `shown`'s viewport and clip them to its clip.
pub(super) fn centred(shown: CanvasPlacement, extent: Extent) -> Centred {
    let (width, height) = (i64::from(extent.width()), i64::from(extent.height()));
    let offset = [
        i64::from(shown.viewport.x) + (i64::from(shown.viewport.width) - width) / 2,
        i64::from(shown.viewport.y) + (i64::from(shown.viewport.height) - height) / 2,
    ];
    let left = (i64::from(shown.clip.x) - offset[0]).clamp(0, width);
    let top = (i64::from(shown.clip.y) - offset[1]).clamp(0, height);
    let right =
        (i64::from(shown.clip.x) + i64::from(shown.clip.width) - offset[0]).clamp(left, width);
    let bottom =
        (i64::from(shown.clip.y) + i64::from(shown.clip.height) - offset[1]).clamp(top, height);
    #[allow(clippy::cast_precision_loss)]
    let clip = D2D_RECT_F {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    };
    Centred { offset, clip }
}

impl Canvas {
    /// Move the committed pixels to `shown` without uploading or acknowledging anything; the
    /// acknowledged frame and placement are unchanged, so input routing stays withdrawn until
    /// a frame at `shown` is committed. Returns whether a compositor commit occurred.
    pub(super) fn reposition(&mut self, shown: CanvasPlacement) -> Result<bool, DesktopError> {
        if self.displayed == Some(shown) {
            return Ok(false);
        }
        let extent = self.active.as_ref().ok_or_else(|| missing("canvas_allocation"))?.extent();
        let centred = centred(shown, extent);
        let visual = self.visual.as_ref().ok_or_else(|| missing("canvas_visual"))?;
        let composition = self.composition.as_ref().ok_or_else(|| missing("canvas_composition"))?;
        // SAFETY: The retained visual and device are live on their creating UI apartment.
        unsafe {
            #[allow(clippy::cast_precision_loss)]
            visual
                .SetOffsetX2(centred.offset[0] as f32)
                .map_err(|cause| DesktopError::new("reposition_canvas_surface", cause))?;
            #[allow(clippy::cast_precision_loss)]
            visual
                .SetOffsetY2(centred.offset[1] as f32)
                .map_err(|cause| DesktopError::new("reposition_canvas_surface", cause))?;
            visual
                .SetClip2(&centred.clip)
                .map_err(|cause| DesktopError::new("clip_repositioned_canvas", cause))?;
            composition
                .Commit()
                .map_err(|cause| DesktopError::new("commit_canvas_reposition", cause))?;
        }
        self.displayed = Some(shown);
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
