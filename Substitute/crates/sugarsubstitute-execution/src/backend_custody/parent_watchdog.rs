// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound coordinator silence using active system time, independently of wall-clock sleep.
use std::time::Duration;

/// Twenty active seconds permit normal scheduling delays and bounded shutdown work.
const SILENCE_TICKS: u64 = 200_000_000;

/// Owned only by the authenticated coordinator process observation.
#[derive(Default)]
pub(super) struct ParentWatchdog {
    observed: Option<u64>,
    expired: bool,
}
impl ParentWatchdog {
    /// Fresh authenticated progress cannot revive an already expired owner.
    pub(super) fn observe(&mut self, active_ticks: u64) {
        if !self.expired {
            self.observed = Some(active_ticks);
        }
    }
    /// Return the next check interval in active time; zero irreversibly expires the lease.
    pub(super) fn remaining(&mut self, active_ticks: u64) -> Option<Duration> {
        let previous = self.observed?;
        let elapsed = active_ticks.saturating_sub(previous);
        self.expired |= elapsed >= SILENCE_TICKS;
        Some(if self.expired {
            Duration::ZERO
        } else {
            Duration::from_nanos((SILENCE_TICKS - elapsed) * 100)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unarmed_observation_cannot_expire_or_authorize_termination() {
        assert_eq!(ParentWatchdog::default().remaining(u64::MAX), None);
    }
    #[test]
    fn fresh_progress_renews_only_before_expiration() {
        let mut owner = ParentWatchdog::default();
        owner.observe(10);
        assert_eq!(owner.remaining(10 + SILENCE_TICKS - 1), Some(Duration::from_nanos(100)));
        owner.observe(10 + SILENCE_TICKS - 1);
        assert_eq!(
            owner.remaining(10 + SILENCE_TICKS),
            Some(Duration::from_nanos((SILENCE_TICKS - 1) * 100))
        );
        assert_eq!(owner.remaining(10 + 2 * SILENCE_TICKS - 1), Some(Duration::ZERO));
        owner.observe(10 + 2 * SILENCE_TICKS);
        assert_eq!(owner.remaining(10 + 2 * SILENCE_TICKS), Some(Duration::ZERO));
    }
    #[test]
    fn sleep_does_not_advance_the_supplied_active_time() {
        let mut owner = ParentWatchdog::default();
        owner.observe(50);
        for _ in 0..3 {
            assert_eq!(owner.remaining(50), Some(Duration::from_secs(20)));
        }
        assert_eq!(owner.remaining(49), Some(Duration::from_secs(20)));
    }
}
