// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Admit real time to tests and native qualification only where it cannot decide an outcome.
//!
//! Tests and qualification campaigns wait for observed outcomes and use deterministic clocks.
//! Real time enters them through this module alone, in two forms:
//!
//! - [`anchor`] is one frozen instant per process. Tests derive every moment they need by adding
//!   durations to it, so a slow or loaded machine never moves a test's notion of time.
//! - [`ProgressGuard`] turns a lost event into a diagnosed failure instead of a hang. It fails only
//!   after nothing it watches has made progress for [`STALL`]; a slow but progressing run only
//!   takes longer. Elapsed time since a wait began never decides anything.

use std::{
    error::Error,
    fmt,
    sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError},
    time::{Duration, Instant},
};

/// Silence that proves an event was lost rather than slow. It is far above the longest single
/// step seen on a starved machine, and any progress resets it.
pub const STALL: Duration = Duration::from_secs(10 * 60);

/// Longest interval a blocked waiter sleeps before it re-examines progress. It decides nothing.
const PATROL: Duration = Duration::from_secs(15);

/// Return the process's frozen test anchor, sampled once on first use.
///
/// The anchor never advances. Code that needs a later moment adds a duration to it.
pub fn anchor() -> Instant {
    static ANCHOR: OnceLock<Instant> = OnceLock::new();
    *ANCHOR.get_or_init(Instant::now)
}

/// A watched run made no progress for its whole stall interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stall {
    /// How long nothing watched had progressed when the guard gave up.
    pub silence: Duration,
}

impl fmt::Display for Stall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "nothing made progress for {:?}", self.silence)
    }
}

impl Error for Stall {}

/// The guard returned by [`ProgressGuard::wait_while`], with the stall that ended the wait.
///
/// A stalled wait still returns the lock so the caller can report the state it never left.
pub struct Watched<'a, T> {
    /// The reacquired lock.
    pub guard: MutexGuard<'a, T>,
    /// `Some` when the wait ended because nothing progressed, not because its condition cleared.
    pub stalled: Option<Stall>,
}

/// Record when anything watched last made progress, and fail only after a whole stall of silence.
///
/// One guard may be shared by every waiter in a process, so parallel work keeps it fresh for as
/// long as any of it progresses. Silence is counted from the latest progress, or from the first
/// observation when nothing has progressed yet.
pub struct ProgressGuard {
    stall: Duration,
    origin: OnceLock<Instant>,
    last: Mutex<Option<Instant>>,
}

impl ProgressGuard {
    /// Create a guard that fails after `stall` of silence.
    pub const fn new(stall: Duration) -> Self {
        Self { stall, origin: OnceLock::new(), last: Mutex::new(None) }
    }

    /// The guard every waiter of this process shares, failing after [`STALL`].
    pub fn process() -> &'static Self {
        static PROCESS: ProgressGuard = ProgressGuard::new(STALL);
        &PROCESS
    }

    /// Record progress now.
    pub fn note(&self) {
        let now = Instant::now();
        self.origin.get_or_init(|| now);
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Some(now);
    }

    fn since(&self) -> Instant {
        let last = *self.last.lock().unwrap_or_else(PoisonError::into_inner);
        last.unwrap_or_else(|| *self.origin.get_or_init(Instant::now))
    }

    /// Time since the latest progress, or since the first observation when none was noted.
    pub fn silence(&self) -> Duration {
        self.since().elapsed()
    }

    /// The instant at which continued silence becomes a stall, for event loops that must wake.
    pub fn deadline(&self) -> Instant {
        let since = self.since();
        since.checked_add(self.stall).unwrap_or(since)
    }

    /// Fail once nothing has progressed for the whole stall interval.
    ///
    /// # Errors
    /// Returns [`Stall`] with the observed silence.
    pub fn check(&self) -> Result<(), Stall> {
        let silence = self.silence();
        if silence >= self.stall { Err(Stall { silence }) } else { Ok(()) }
    }

    /// Block on `changed` while `waiting` holds, until it clears or nothing progresses for the
    /// stall interval. A cleared condition counts as progress.
    ///
    /// Poisoning is ignored: the waiter's own state is what it inspects next.
    pub fn wait_while<'a, T>(
        &self,
        changed: &Condvar,
        mut guard: MutexGuard<'a, T>,
        mut waiting: impl FnMut(&mut T) -> bool,
    ) -> Watched<'a, T> {
        self.since();
        let patrol = PATROL.min(self.stall / 4);
        loop {
            let (next, result) = changed
                .wait_timeout_while(guard, patrol, &mut waiting)
                .unwrap_or_else(PoisonError::into_inner);
            guard = next;
            if !result.timed_out() {
                self.note();
                return Watched { guard, stalled: None };
            }
            if let Err(stall) = self.check() {
                return Watched { guard, stalled: Some(stall) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A wait whose event never arrives ends as a stall that still returns the lock.
    #[test]
    fn a_wait_without_progress_ends_as_a_stall() {
        let guard = ProgressGuard::new(Duration::from_millis(40));
        let changed = Condvar::new();
        let state = Mutex::new(false);
        let watched = guard.wait_while(
            &changed,
            state.lock().unwrap_or_else(PoisonError::into_inner),
            |released| !*released,
        );
        assert!(!*watched.guard);
        assert!(watched.stalled.is_some_and(|stall| stall.silence >= Duration::from_millis(40)));
        assert!(guard.check().is_err());
    }

    /// A cleared condition returns at once and counts as progress.
    #[test]
    fn a_cleared_condition_returns_and_resets_the_silence() -> Result<(), Stall> {
        let guard = ProgressGuard::new(STALL);
        let shared = Arc::new((Mutex::new(true), Condvar::new()));
        let watched = guard.wait_while(
            &shared.1,
            shared.0.lock().unwrap_or_else(PoisonError::into_inner),
            |released| !*released,
        );
        assert!(watched.stalled.is_none());
        drop(watched);
        assert!(guard.deadline() > anchor());
        guard.check()
    }

    /// The anchor is sampled once and never advances.
    #[test]
    fn the_anchor_is_frozen() {
        let first = anchor();
        assert_eq!(anchor(), first);
    }

    /// A zero stall reports silence immediately, without any wait.
    #[test]
    fn a_zero_stall_fails_its_first_check() {
        let guard = ProgressGuard::new(Duration::ZERO);
        assert!(guard.check().is_err());
    }
}
