// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample one bounded camera transition from elapsed time with exact final coordinates.
//!
//! Every camera change follows one curve: a quartic ease-out over `FOLLOW`, which starts at four
//! times the average speed, so the first display frame already covers a large part of the way,
//! and stops with zero velocity exactly at its target. A new request re-aims from wherever the
//! camera is, so rapid wheel input keeps the camera moving close behind the latest target instead
//! of jumping or trailing. Positions are sampled from elapsed time, so a late frame shows the
//! current point on the curve. Only a request that leaves the scale unchanged lands at once.
use super::*;

/// Length of every camera motion.
const FOLLOW: Duration = Duration::from_millis(80);

#[derive(Clone, Copy, Debug)]
pub(super) struct Transition {
    pub(super) start: Viewport,
    pub(super) target: Viewport,
    pub(super) begin: Duration,
    pub(super) end: Duration,
    pub(super) anchor: Option<[f64; 2]>,
}
impl Navigation {
    pub(super) fn advance_inner(&mut self, now: Duration) -> Result<(), CanvasError> {
        self.observe_time(now)?;
        if self.wheel_end.is_some_and(|end| now >= end) {
            self.wheel_end = None;
            self.wheel_active = false;
        }
        let Some(transition) = self.transition else {
            return Ok(());
        };
        if now >= transition.end {
            self.view = transition.target;
            self.transition = None;
            return Ok(());
        }
        let t = (now - transition.begin).as_secs_f64()
            / (transition.end - transition.begin).as_secs_f64();
        let eased = 1.0 - (1.0 - t).powi(4);
        // Scale moves geometrically, so zooming in and out start equally fast on screen.
        let zoom = transition.start.zoom()
            * (transition.target.zoom() / transition.start.zoom()).powf(eased);
        let origin = if let Some(anchor) = transition.anchor {
            let point = transition.start.document_at(anchor)?;
            [point[0] - anchor[0] / zoom, point[1] - anchor[1] / zoom]
        } else {
            let start = transition.start.origin();
            let end = transition.target.origin();
            let size = [self.view.surface().width(), self.view.surface().height()];
            let content =
                self.content.ok_or(CanvasError::Invalid("Camera motion has no content"))?;
            let image = [content.width(), content.height()];
            std::array::from_fn(|axis| {
                let center = f64::from(image[axis]) * 0.5;
                let panel = f64::from(size[axis]) * 0.5;
                let start_pan = (center - start[axis]) * transition.start.zoom() - panel;
                let end_pan = (center - end[axis]) * transition.target.zoom() - panel;
                let pan = start_pan + (end_pan - start_pan) * eased;
                center - (panel + pan) / zoom
            })
        };
        self.view.reframe(origin, zoom)?;
        self.view = self.clamp(self.view)?;
        Ok(())
    }
    /// Let an explicit physical camera request take ownership under interactive bounds.
    /// # Errors
    /// Preserves coordinate, scale and transition deadline failures.
    pub(super) fn transition_to(
        &mut self,
        zoom: f64,
        mode: ZoomMode,
        anchor: Option<[f64; 2]>,
        cadence: Option<Duration>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        self.restored_camera = false;
        self.mode = mode;
        let mut target = self.view;
        if mode == ZoomMode::Fit {
            if let Some(content) = self.content {
                target.fit(content)?;
            }
        } else {
            let anchor = anchor.unwrap_or([
                f64::from(target.surface().width()) * 0.5,
                f64::from(target.surface().height()) * 0.5,
            ]);
            let point = target.document_at(anchor)?;
            target.reframe([point[0] - anchor[0] / zoom, point[1] - anchor[1] / zoom], zoom)?;
        }
        self.transition_target(target, mode, anchor, cadence, now)
    }
    /// Share one interpolation policy between physical input and semantic commands.
    pub(super) fn transition_target(
        &mut self,
        target: Viewport,
        mode: ZoomMode,
        anchor: Option<[f64; 2]>,
        cadence: Option<Duration>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        self.mode = mode;
        let target = self.clamp(target)?;
        let snapped =
            ((target.zoom() - self.view.zoom()).abs() <= 1e-4).then_some(ZoomSnap::Unchanged);
        let duration = if snapped.is_some() { Duration::ZERO } else { FOLLOW };
        self.zoom = Some(ZoomMotion {
            gap: cadence,
            duration: if snapped.is_some() { Duration::ZERO } else { duration },
            snapped,
        });
        if snapped.is_some() {
            self.view = target;
            self.transition = None;
        } else {
            self.transition = Some(Transition {
                start: self.view,
                target,
                begin: now,
                end: now
                    .checked_add(duration)
                    .ok_or(CanvasError::Invalid("Navigation deadline overflow"))?,
                anchor: if mode == ZoomMode::Fit { None } else { anchor },
            });
        }
        Ok(())
    }
}
