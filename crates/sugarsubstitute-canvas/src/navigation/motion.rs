// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample one bounded camera transition from elapsed time with exact final coordinates.
use super::*;

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
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        let zoom =
            transition.start.zoom() + (transition.target.zoom() - transition.start.zoom()) * eased;
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
    pub(super) fn transition_to(
        &mut self,
        zoom: f64,
        mode: ZoomMode,
        anchor: Option<[f64; 2]>,
        cadence: Option<Duration>,
        now: Duration,
    ) -> Result<(), CanvasError> {
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
        target = self.clamp(target)?;
        let duration = match cadence.map(|time| time.as_secs_f64() * 1000.0) {
            Some(ms) if ms <= 25.0 => 0.0,
            Some(ms) if ms < 160.0 => 20.0 + 60.0 * ((ms - 40.0) / 120.0).clamp(0.0, 1.0),
            _ => 80.0,
        };
        let duration = Duration::from_secs_f64(duration / 1000.0);
        if duration < self.frame_period || (target.zoom() - self.view.zoom()).abs() <= 1e-4 {
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
