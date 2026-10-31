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
        };
        next.transition_target(target, mode, None, None, now)?;
        let target = next.target_viewport();
        *self = next;
        Ok(NavigationCommandOutcome::Applied { target })
    }
}
