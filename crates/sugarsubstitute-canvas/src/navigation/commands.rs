// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit semantic camera commands without manufacturing pointer or gesture observations.

use super::*;

impl Navigation {
    /// Stop gestures and motion at a camera already accepted by the presentation owner.
    /// The caller qualifies this camera against the current document and source before use.
    /// Coordinates are document pixels and scale is physical pixels per document pixel.
    /// A completed destination retains its semantic mode; an interrupted destination is Custom.
    /// No elapsed motion is sampled beyond the supplied accepted camera.
    ///
    /// # Errors
    /// Rejects absent content, foreign physical bounds or backward time without mutation.
    pub fn freeze_at(&mut self, accepted: Viewport, now: Duration) -> Result<(), CanvasError> {
        if self.content.is_none() || accepted.surface() != self.view.surface() {
            return Err(CanvasError::Invalid("Accepted camera does not match navigation content"));
        }
        let mut next = *self;
        next.observe_time(now)?;
        let mode = if next.target_viewport() == accepted { next.mode } else { ZoomMode::Custom };
        next.cancel();
        next.view = accepted;
        next.mode = mode;
        *self = next;
        Ok(())
    }

    /// Stop only a matching semantic destination at its currently sampled camera.
    /// Physical capture/wheel gestures retain ownership, and a newer destination is untouched.
    /// Already sampled motion is preserved; stopping partway through a stop becomes Custom.
    ///
    /// # Errors
    /// Rejects backward time or invalid sampled geometry without changing navigation.
    pub fn cancel_command(
        &mut self,
        target: Viewport,
        mode: ZoomMode,
        now: Duration,
    ) -> Result<bool, CanvasError> {
        if self.dragging
            || self.wheel_active
            || self.target_viewport() != target
            || self.mode != mode
        {
            return Ok(false);
        }
        let mut next = *self;
        next.advance_inner(now)?;
        next.cancel();
        *self = next;
        Ok(true)
    }

    /// Apply an image-centered stop, direct scale, or physical viewport-relative pan.
    /// Active pointer/wheel gestures and host policy retain ownership without cancellation.
    /// An admitted command supersedes interpolation from its currently sampled camera.
    ///
    /// # Errors
    /// Rejects nonfinite/out-of-range parameters, backward time or invalid geometry atomically.
    pub fn command(
        &mut self,
        command: NavigationCommand,
        now: Duration,
    ) -> Result<NavigationCommandOutcome, CanvasError> {
        match command {
            NavigationCommand::Zoom(factor)
                if !factor.is_finite() || !(0.01..=100.0).contains(&factor) =>
            {
                return Err(CanvasError::Invalid("Semantic zoom exceeds camera bounds"));
            }
            NavigationCommand::Pan(delta)
                if delta
                    .iter()
                    .any(|value| !value.is_finite() || !(-10.0..=10.0).contains(value)) =>
            {
                return Err(CanvasError::Invalid("Semantic pan exceeds viewport bounds"));
            }
            NavigationCommand::Frame { center, span }
                if center.iter().any(|value| !value.is_finite())
                    || span.iter().any(|value| {
                        !value.is_finite() || *value <= 0.0 || *value > MAXIMUM_FRAME_SPAN
                    }) =>
            {
                return Err(CanvasError::Invalid("Semantic frame exceeds camera bounds"));
            }
            _ => {}
        }
        let mut next = *self;
        next.advance_inner(now)?;
        let Some(content) = next.content else {
            return Ok(NavigationCommandOutcome::NoContent);
        };
        if next.locked || next.drag_out || next.dragging || next.wheel_active {
            return Ok(NavigationCommandOutcome::Denied);
        }
        let mut target = match command {
            NavigationCommand::Zoom(_) | NavigationCommand::Pan(_) => next.target_viewport(),
            _ => next.view,
        };
        let previous_mode = next.mode;
        next.cancel();
        if let NavigationCommand::Frame { center, span } = command {
            next.frame(target, content, center, span)?;
            let target = next.target_viewport();
            *self = next;
            return Ok(NavigationCommandOutcome::Applied { target });
        }
        next.restored_camera = false;
        let mode = match command {
            NavigationCommand::Fit => {
                target.fit(content)?;
                ZoomMode::Fit
            }
            NavigationCommand::OneToOne => {
                target.reframe(
                    [
                        (f64::from(content.width()) - f64::from(target.surface().width())) * 0.5,
                        (f64::from(content.height()) - f64::from(target.surface().height())) * 0.5,
                    ],
                    1.0,
                )?;
                ZoomMode::OneToOne
            }
            NavigationCommand::Zoom(factor) => {
                let zoom = next.wheel_scale(target.zoom() * factor);
                let center = [
                    f64::from(target.surface().width()) * 0.5,
                    f64::from(target.surface().height()) * 0.5,
                ];
                let point = target.document_at(center)?;
                if zoom != target.zoom() {
                    target.reframe(
                        [point[0] - center[0] / zoom, point[1] - center[1] / zoom],
                        zoom,
                    )?;
                }
                ZoomMode::Custom
            }
            NavigationCommand::Pan(fraction) => {
                target.pan_by([
                    fraction[0] * f64::from(target.surface().width()),
                    fraction[1] * f64::from(target.surface().height()),
                ])?;
                previous_mode
            }
            NavigationCommand::Frame { .. } => {
                return Err(CanvasError::Invalid("Semantic frame bypassed its immediate owner"));
            }
        };
        next.transition_target(target, mode, None, None, now)?;
        let target = next.target_viewport();
        *self = next;
        Ok(NavigationCommandOutcome::Applied { target })
    }
}

/// The inclusive span ceiling: one content extent per physical pixel at the smallest scale.
const MAXIMUM_FRAME_SPAN: f64 = 1e9;

impl Navigation {
    /// Restore a numerically representable image-relative camera without interactive bounds.
    /// The caller has already cancelled gestures and motion and validated the parameters.
    fn frame(
        &mut self,
        mut target: Viewport,
        content: Extent,
        center: [f64; 2],
        span: [f64; 2],
    ) -> Result<(), CanvasError> {
        let image = [f64::from(content.width()), f64::from(content.height())];
        let mut fitted = target;
        fitted.fit(content)?;
        let requested = (1.0 / (image[0] * span[0])).min(1.0 / (image[1] * span[1]));
        let panel =
            [f64::from(target.surface().width()) * 0.5, f64::from(target.surface().height()) * 0.5];
        target.reframe(
            [
                center[0] * image[0] - panel[0] / requested,
                center[1] * image[1] - panel[1] / requested,
            ],
            requested,
        )?;
        target.device_at([0.0; 2])?;
        target.device_at(image)?;
        target.document_at([
            f64::from(target.surface().width()),
            f64::from(target.surface().height()),
        ])?;
        self.mode = if target == fitted {
            ZoomMode::Fit
        } else if requested == 1.0 {
            ZoomMode::OneToOne
        } else {
            ZoomMode::Custom
        };
        self.view = target;
        self.restored_camera = true;
        Ok(())
    }
}
