// SPDX-License-Identifier: AGPL-3.0-or-later
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
    /// Sample active time from `clock` and end the coordinator through `terminate` once it has
    /// been silent for the whole lease; returns [`Self::remaining`]. Active time stands still
    /// while the system is suspended, so a sleep of any length neither shortens the lease nor
    /// ends a coordinator, and a clock that cannot be read ends nothing.
    pub(super) fn enforce<E>(
        &mut self,
        clock: impl FnOnce() -> Result<u64, E>,
        terminate: impl FnOnce() -> Result<(), E>,
    ) -> Result<Option<Duration>, E> {
        let remaining = self.remaining(clock()?);
        if remaining == Some(Duration::ZERO) {
            terminate()?;
        }
        Ok(remaining)
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

    const SECOND: u64 = 10_000_000;

    fn enforce(
        owner: &mut ParentWatchdog,
        active_ticks: u64,
        ended: &std::cell::Cell<u32>,
    ) -> Result<Option<Duration>, &'static str> {
        owner.enforce(
            || Ok(active_ticks),
            || {
                ended.set(ended.get() + 1);
                Ok(())
            },
        )
    }

    /// A trial's coordinator that is silent because the system sleeps is not unresponsive: the
    /// lease is read on the clock that stands still during a suspend, so the coordinator is
    /// never ended for it, and the lease goes on from where it stood once the system resumes.
    #[test]
    fn a_suspend_of_any_length_never_ends_the_coordinator_and_the_lease_resumes_where_it_stood()
    -> Result<(), &'static str> {
        let ended = std::cell::Cell::new(0);
        let mut owner = ParentWatchdog::default();
        owner.observe(1_000);
        let before_sleep = 1_000 + 5 * SECOND;
        assert_eq!(enforce(&mut owner, before_sleep, &ended)?, Some(Duration::from_secs(15)));
        // Eight hours asleep pass on the wall clock; active time has not moved at any check
        // the resumed custodian makes.
        for _ in 0..3 {
            assert_eq!(enforce(&mut owner, before_sleep, &ended)?, Some(Duration::from_secs(15)));
        }
        assert_eq!(ended.get(), 0);
        // Progress after the resume renews the lease as it always does.
        owner.observe(before_sleep + SECOND);
        assert_eq!(
            enforce(&mut owner, before_sleep + 2 * SECOND, &ended)?,
            Some(Duration::from_secs(19))
        );
        // Twenty active seconds of silence still end a coordinator that really hangs, once.
        assert_eq!(enforce(&mut owner, before_sleep + 21 * SECOND, &ended)?, Some(Duration::ZERO));
        assert_eq!(ended.get(), 1);
        Ok(())
    }

    #[test]
    fn a_clock_that_cannot_be_read_ends_nothing() {
        let ended = std::cell::Cell::new(0);
        let mut owner = ParentWatchdog::default();
        owner.observe(0);
        let unread = owner.enforce(
            || Err("clock"),
            || {
                ended.set(ended.get() + 1);
                Ok(())
            },
        );
        assert_eq!((unread, ended.get()), (Err("clock"), 0));
    }
}
