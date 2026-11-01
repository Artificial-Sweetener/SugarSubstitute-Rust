// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own semantic navigation and time-based camera motion without scheduling or native handles.
mod actions;
mod bounds;
mod commands;
mod motion;
mod types;
use crate::{CanvasError, Viewport};
use motion::Transition;
use std::time::Duration;
use sugarsubstitute_image_ops::Extent;
pub use types::{
    CursorIntent, NavigationCommand, NavigationCommandOutcome, NavigationInput, ScrollPhase,
    ZoomMode,
};

/// Portable navigation state. The application supplies monotonic elapsed time from its clock.
/// Native pumping and rendering jobs remain with their existing owners; no timer is created here.
#[derive(Clone, Copy, Debug)]
pub struct Navigation {
    view: Viewport,
    content: Option<Extent>,
    mode: ZoomMode,
    pointer: [f64; 2],
    dragging: bool,
    locked: bool,
    drag_out: bool,
    wheel_active: bool,
    wheel_end: Option<Duration>,
    last_request: Option<Duration>,
    transition: Option<Transition>,
    frame_period: Duration,
    last_time: Option<Duration>,
}
impl Navigation {
    /// Create inert navigation for a drawable camera with no loaded content.
    pub const fn new(view: Viewport) -> Self {
        Self {
            view,
            content: None,
            mode: ZoomMode::Fit,
            pointer: [0.0; 2],
            dragging: false,
            locked: false,
            drag_out: false,
            wheel_active: false,
            wheel_end: None,
            last_request: None,
            transition: None,
            frame_period: Duration::from_nanos(16_666_667),
            last_time: None,
        }
    }
    /// Actual current camera, distinct from the pending transition target.
    pub const fn viewport(&self) -> Viewport {
        self.view
    }
    /// Semantic destination mode, including while a transition is still in progress.
    pub const fn zoom_mode(&self) -> ZoomMode {
        self.mode
    }
    /// The final requested camera; subsequent wheel input accumulates against this intent.
    pub fn target_viewport(&self) -> Viewport {
        self.transition.map_or(self.view, |transition| transition.target)
    }
    /// Whether pointer capture is currently held by navigation.
    pub const fn captured(&self) -> bool {
        self.dragging
    }
    /// Whether the gesture or interpolation still requires interactive presentation quality.
    pub const fn interacting(&self) -> bool {
        self.dragging || self.wheel_active || self.transition.is_some()
    }
    /// Whether wheel navigation or camera interpolation changes the sampling scale.
    /// A direct pan can preserve settled pixels through exact integer blits and edge repair.
    pub const fn resampling(&self) -> bool {
        self.wheel_active || self.transition.is_some()
    }
    /// Current monitor's fractional frame period, including the portable 60 Hz fallback.
    pub const fn frame_period(&self) -> Duration {
        self.frame_period
    }
    /// Whether a primary drag should export the content instead of panning: unlocked content
    /// that cannot pan, because Fit or its zoom keeps it within the surface on both axes.
    pub fn allows_drag_out(&self) -> bool {
        self.content.is_some() && !self.locked && !self.can_pan()
    }
    /// Resolve feedback from the same availability rules that admit pan gestures.
    pub fn cursor(&self) -> CursorIntent {
        if self.dragging && !self.drag_out && self.can_pan() {
            CursorIntent::ClosedHand
        } else if !self.drag_out && self.can_pan() {
            CursorIntent::OpenHand
        } else {
            CursorIntent::Arrow
        }
    }
    /// Replace source geometry and cancel old gestures; a loaded image starts in Fit.
    /// # Errors
    /// Rejects backward time or an unsupported fitted camera without mutation.
    pub fn set_content(
        &mut self,
        content: Option<Extent>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let mut next = *self;
        next.observe_time(now)?;
        next.cancel();
        next.content = content;
        next.mode = ZoomMode::Fit;
        if let Some(extent) = content {
            next.view.fit(extent)?;
        }
        *self = next;
        Ok(())
    }
    /// Update physical viewport bounds, preserving manual centers and refitting semantic Fit.
    /// # Errors
    /// Rejects backward time or invalid geometry without mutation.
    pub fn resize(&mut self, extent: Extent, now: Duration) -> Result<(), CanvasError> {
        let mut next = *self;
        next.advance_inner(now)?;
        if next.view.surface() != extent {
            let fitting = next.mode == ZoomMode::Fit;
            let transition = next.transition;
            next.view.resize(extent)?;
            if transition.is_none() {
                if fitting && let Some(content) = next.content {
                    next.view.fit(content)?;
                } else {
                    next.view = next.clamp(next.view)?;
                }
            } else if let Some(mut transition) = transition {
                next.view = next.clamp(next.view)?;
                transition.start = next.view;
                transition.target.resize(extent)?;
                if fitting && let Some(content) = next.content {
                    transition.target.fit(content)?;
                } else {
                    transition.target = next.clamp(transition.target)?;
                }
                transition.begin = now;
                next.transition = (transition.target != transition.start).then_some(transition);
            }
            next.release_pan_if_unavailable();
        }
        *self = next;
        Ok(())
    }
    /// Set navigation permission and host drag-out ownership; capture is cancelled when unavailable.
    pub fn set_policy(&mut self, locked: bool, drag_out: bool) {
        self.locked = locked;
        self.drag_out = drag_out;
        self.release_pan_if_unavailable();
    }
    /// Set the active display rate, retaining fractional frame periods and current motion progress.
    /// `None` uses 60 Hz. The desktop resolves actual display timing and monitor changes.
    /// # Errors
    /// Rejects nonfinite/nonpositive rates or unsupported frame periods without mutation.
    pub fn set_refresh_rate(&mut self, hz: Option<f64>) -> Result<(), CanvasError> {
        let hz = hz.unwrap_or(60.0);
        if !hz.is_finite() || !(1.0..=1000.0).contains(&hz) {
            return Err(CanvasError::Invalid("Invalid display refresh rate"));
        }
        self.frame_period = Duration::try_from_secs_f64(1.0 / hz)
            .map_err(|_| CanvasError::Invalid("Invalid display frame period"))?;
        Ok(())
    }
    /// Earliest desired animation/idle wake. Missed ticks sample current time without a backlog.
    /// The next display tick is anchored to the transition start and bounded by its exact end.
    pub fn next_deadline(&self, now: Duration) -> Option<Duration> {
        let animation = self.transition.map(|transition| {
            let elapsed = now.saturating_sub(transition.begin).as_nanos();
            let remainder = Duration::from_nanos((elapsed % self.frame_period.as_nanos()) as u64);
            now.saturating_add(self.frame_period - remainder).min(transition.end)
        });
        match (animation, self.wheel_end) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    /// Apply one normalized observation at monotonic time, atomically on invalid input.
    /// # Errors
    /// Rejects nonfinite input, backward time, invalid camera geometry or deadline overflow.
    pub fn input(&mut self, input: NavigationInput, now: Duration) -> Result<(), CanvasError> {
        let mut next = *self;
        next.advance_inner(now)?;
        next.release_pan_if_unavailable();
        next.apply(input, now)?;
        next.release_pan_if_unavailable();
        *self = next;
        Ok(())
    }
    /// Sample time-based motion and idle expiration; late ticks reach the exact target.
    /// # Errors
    /// Rejects backward time or invalid camera geometry without mutation.
    pub fn advance(&mut self, now: Duration) -> Result<(), CanvasError> {
        let mut next = *self;
        next.advance_inner(now)?;
        next.release_pan_if_unavailable();
        *self = next;
        Ok(())
    }
    fn observe_time(&mut self, now: Duration) -> Result<(), CanvasError> {
        if self.last_time.is_some_and(|previous| now < previous) {
            return Err(CanvasError::Invalid("Navigation clock moved backwards"));
        }
        self.last_time = Some(now);
        Ok(())
    }
    fn cancel(&mut self) {
        if self.transition.is_some_and(|transition| transition.target != self.view) {
            self.mode = ZoomMode::Custom;
        }
        self.dragging = false;
        self.wheel_active = false;
        self.wheel_end = None;
        self.last_request = None;
        self.transition = None;
    }
}
#[cfg(test)]
mod tests;
