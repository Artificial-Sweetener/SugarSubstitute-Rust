// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own semantic navigation and time-based camera motion without scheduling or native handles.
mod actions;
mod bounds;
mod motion;
mod types;
use crate::{CanvasError, Viewport};
use motion::Transition;
use std::time::Duration;
use sugarsubstitute_image_ops::Extent;
pub use types::{CursorIntent, NavigationInput, ScrollPhase, ZoomMode};

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
    /// Resolve feedback from the same availability rules that admit pan gestures.
    pub fn cursor(&self) -> CursorIntent {
        if self.dragging {
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
            next.cancel();
            next.view.resize(extent)?;
            if next.mode == ZoomMode::Fit
                && let Some(content) = next.content
            {
                next.view.fit(content)?;
            }
        }
        *self = next;
        Ok(())
    }
    /// Set navigation permission and host drag-out ownership; capture is cancelled when unavailable.
    pub fn set_policy(&mut self, locked: bool, drag_out: bool) {
        self.locked = locked;
        self.drag_out = drag_out;
        if locked || drag_out {
            self.cancel();
        }
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
    /// Earliest desired animation/idle wake. Missed ticks are sampled at current time, never queued.
    /// # Errors
    /// Rejects deadline overflow; does not create a scheduled task.
    pub fn next_deadline(&self, now: Duration) -> Result<Option<Duration>, CanvasError> {
        let animation = self
            .transition
            .map(|transition| {
                let elapsed = now.saturating_sub(transition.begin).as_nanos();
                let period = self.frame_period.as_nanos();
                let remaining = u64::try_from(period - elapsed % period)
                    .map_err(|_| CanvasError::Invalid("Navigation period overflow"))?;
                now.checked_add(Duration::from_nanos(remaining))
                    .map(|tick| tick.min(transition.end))
                    .ok_or(CanvasError::Invalid("Navigation deadline overflow"))
            })
            .transpose()?;
        Ok(match (animation, self.wheel_end) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        })
    }
    /// Apply one normalized observation at monotonic time, atomically on invalid input.
    /// # Errors
    /// Rejects nonfinite input, backward time, invalid camera geometry or deadline overflow.
    pub fn input(&mut self, input: NavigationInput, now: Duration) -> Result<(), CanvasError> {
        let mut next = *self;
        next.advance_inner(now)?;
        next.apply(input, now)?;
        *self = next;
        Ok(())
    }
    /// Sample time-based motion and idle expiration; late ticks reach the exact target.
    /// # Errors
    /// Rejects backward time or invalid camera geometry without mutation.
    pub fn advance(&mut self, now: Duration) -> Result<(), CanvasError> {
        let mut next = *self;
        next.advance_inner(now)?;
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
        self.dragging = false;
        self.wheel_active = false;
        self.wheel_end = None;
        self.last_request = None;
        self.transition = None;
    }
}
#[cfg(test)]
mod tests;
