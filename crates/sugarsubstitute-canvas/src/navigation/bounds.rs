// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share availability and clamping rules across dragging and animated camera updates.
use super::*;

impl Navigation {
    /// Whether unlocked, non-Fit content extends beyond the drawable surface on either axis.
    pub fn can_pan(&self) -> bool {
        let Some(content) = self.content else {
            return false;
        };
        if self.locked || self.mode == ZoomMode::Fit {
            return false;
        }
        let image = [content.width(), content.height()];
        let panel = [self.view.surface().width(), self.view.surface().height()];
        (0..2).any(|axis| f64::from(image[axis]) * self.view.zoom() > f64::from(panel[axis]))
    }
    /// Clamp content-centered physical displacement without rounding a valid sampling lattice.
    pub(super) fn clamp(&self, mut view: Viewport) -> Result<Viewport, CanvasError> {
        let Some(content) = self.content else {
            return Ok(view);
        };
        if self.mode == ZoomMode::Fit {
            let mut fitted = view;
            fitted.fit(content)?;
            if view.zoom() == fitted.zoom() {
                return Ok(fitted);
            }
        }
        let image = [content.width(), content.height()];
        let panel = [view.surface().width(), view.surface().height()];
        let mut delta = [0.0; 2];
        for axis in 0..2 {
            let scaled = f64::from(image[axis]) * view.zoom();
            let panel = f64::from(panel[axis]);
            let pan = (scaled - panel) * 0.5 - view.origin()[axis] * view.zoom();
            let bound = if scaled <= panel { 0.0 } else { (scaled - panel) * 0.5 + panel * 0.2 };
            delta[axis] = pan.clamp(-bound, bound) - pan;
        }
        if delta != [0.0; 2] {
            view.pan_by(delta)?;
        }
        Ok(view)
    }
    /// Release capture without cancelling an independent zoom transition.
    pub(super) fn release_pan_if_unavailable(&mut self) {
        if self.dragging && (self.drag_out || !self.can_pan()) {
            self.dragging = false;
        }
    }
    /// The UI navigation ceiling is distinct from the camera's larger numerical safety bounds.
    pub(super) fn wheel_scale(&self, requested: f64) -> f64 {
        let minimum = self.content.map_or(0.001, |extent| {
            (128.0 / f64::from(extent.width().max(extent.height()))).max(0.001)
        });
        requested.max(minimum).min(10.0)
    }
}
