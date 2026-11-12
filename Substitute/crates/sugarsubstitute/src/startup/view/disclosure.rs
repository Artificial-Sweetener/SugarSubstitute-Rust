// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample reversible console motion from the host's existing presentation clock.
use std::time::Duration;

/// Own only interpolation; native visibility follows accepted frame publication.
pub(super) struct Disclosure {
    started: Duration,
    from: f64,
    target: f64,
}
impl Disclosure {
    /// Begin with a collapsed console and no scheduled work.
    pub(super) fn new() -> Self {
        Self { started: Duration::ZERO, from: 0.0, target: 0.0 }
    }

    /// Reverse from the current sample without jumping to an endpoint.
    pub(super) fn toggle(&mut self, now: Duration, reduced: bool) -> bool {
        self.from = self.sample(now);
        self.target = 1.0 - self.target;
        self.started = now;
        if reduced {
            self.from = self.target;
        }
        self.target == 1.0
    }

    /// Ease both ends of a 220-millisecond transition without an independent timer.
    pub(super) fn sample(&self, now: Duration) -> f64 {
        let time = (now.saturating_sub(self.started).as_secs_f64() / 0.220).min(1.0);
        let eased = time * time * (3.0 - 2.0 * time);
        self.from + (self.target - self.from) * eased
    }
}
