// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interpret normalized pointer and wheel gestures through one camera policy.
use super::*;

impl Navigation {
    /// Apply one physical observation, retaining an inspection until actual camera input occurs.
    /// Duplicate pointer coordinates during capture do not replace the restored destination.
    /// # Errors
    /// Preserves invalid input, camera geometry and monotonic deadline failures.
    pub(super) fn apply(
        &mut self,
        input: NavigationInput,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if matches!(input, NavigationInput::Cancel) {
            self.cancel();
            return Ok(());
        }
        if let NavigationInput::Pointer(point) | NavigationInput::DoubleClick(point) = input
            && point.iter().any(|coordinate| !coordinate.is_finite())
        {
            return Err(CanvasError::Invalid("Nonfinite navigation pointer"));
        }
        if let NavigationInput::Wheel { steps, .. } = input
            && !steps.is_finite()
        {
            return Err(CanvasError::Invalid("Nonfinite wheel steps"));
        }
        if let NavigationInput::Pointer(point) = input {
            if self.dragging {
                self.transition = None;
                let delta = [point[0] - self.pointer[0], point[1] - self.pointer[1]];
                if delta != [0.0; 2] {
                    self.restored_camera = false;
                    let mut view = self.view;
                    view.pan_by(delta)?;
                    self.view = self.clamp(view)?;
                }
            }
            self.pointer = point;
            return Ok(());
        }
        if matches!(input, NavigationInput::Pan(false)) {
            self.dragging = false;
            return Ok(());
        }
        if self.locked || self.content.is_none() {
            return Ok(());
        }
        match input {
            NavigationInput::Pan(true) if self.can_pan() && !self.drag_out => {
                self.cancel();
                self.dragging = true;
            }
            NavigationInput::Wheel { steps, phase } => self.wheel(steps, phase, now)?,
            NavigationInput::DoubleClick(point) => {
                let mode =
                    if self.mode == ZoomMode::Fit { ZoomMode::OneToOne } else { ZoomMode::Fit };
                self.cancel();
                self.transition_to(1.0, mode, Some(point), None, now)?;
            }
            NavigationInput::Fit | NavigationInput::OneToOne => {
                let mode =
                    if input == NavigationInput::Fit { ZoomMode::Fit } else { ZoomMode::OneToOne };
                self.cancel();
                self.transition_to(1.0, mode, Some(self.pointer), None, now)?;
            }
            _ => {}
        }
        Ok(())
    }
    fn wheel(&mut self, steps: f64, phase: ScrollPhase, now: Duration) -> Result<(), CanvasError> {
        if phase == ScrollPhase::End && steps == 0.0 {
            self.wheel_active = false;
            self.wheel_end = None;
            return Ok(());
        }
        if steps == 0.0 {
            return Ok(());
        }
        let old = self.target_viewport().zoom();
        let requested = self.wheel_scale(
            old * if steps > 0.0 {
                1.25_f64.powf(steps.abs().min(256.0))
            } else {
                0.8_f64.powf(steps.abs().min(256.0))
            },
        );
        let mut fit = self.view;
        if let Some(content) = self.content {
            fit.fit(content)?;
        }
        let crosses =
            |stop: f64| old < stop && stop <= requested || old > stop && stop >= requested;
        let (zoom, mode) = if crosses(1.0) {
            (1.0, ZoomMode::OneToOne)
        } else if crosses(fit.zoom()) {
            (fit.zoom(), ZoomMode::Fit)
        } else {
            (requested, ZoomMode::Custom)
        };
        let cadence = self.last_request.map(|previous| now - previous);
        self.last_request = Some(now);
        self.wheel_active = phase != ScrollPhase::End && !self.dragging;
        self.wheel_end = if phase == ScrollPhase::Unphased {
            Some(
                now.checked_add(Duration::from_millis(120))
                    .ok_or(CanvasError::Invalid("Navigation deadline overflow"))?,
            )
        } else {
            None
        };
        self.transition_to(zoom, mode, Some(self.pointer), cadence, now)
    }
}
