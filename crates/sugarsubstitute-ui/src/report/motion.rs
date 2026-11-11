// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ease the details reveal from caller-supplied elapsed time; no clock is read here.
use std::time::Duration;

/// Length of one details expansion or collapse.
pub const EXPAND_DURATION: Duration = Duration::from_millis(160);

/// One reveal or collapse. Expansion eases out (cubic); collapse eases in (cubic), matching the
/// Fluent enter and exit curves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetailsMotion {
    from: f32,
    expanding: bool,
    started: Duration,
}

impl DetailsMotion {
    /// Start moving from the current expansion toward the requested end at `now`.
    pub fn start(current: f32, expanding: bool, now: Duration) -> Self {
        Self { from: current.clamp(0.0, 1.0), expanding, started: now }
    }

    /// Expansion at `now`, and whether the motion has finished.
    pub fn sample(&self, now: Duration) -> (f32, bool) {
        let target = if self.expanding { 1.0 } else { 0.0 };
        let elapsed = now.saturating_sub(self.started).as_secs_f32();
        let t = (elapsed / EXPAND_DURATION.as_secs_f32()).clamp(0.0, 1.0);
        let eased = if self.expanding { 1.0 - (1.0 - t).powi(3) } else { t.powi(3) };
        (self.from + (target - self.from) * eased, t >= 1.0)
    }
}
