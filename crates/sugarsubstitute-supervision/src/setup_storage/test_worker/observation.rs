// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound native qualification by stalled calls and returned work, independently of domain clocks.
use std::{
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};

/// Cover the largest 8192-entry fixture with at most 32 nested storage returns per entry.
/// Repeated activity cannot keep an erroneous native loop alive without a finite work ceiling.
const MAX_RETURNS: u64 = 8192 * 32;

#[derive(Clone, Copy, Default)]
pub(super) struct Progress {
    pub(super) returned: u64,
    last_return: Duration,
}
#[derive(Default)]
struct State {
    progress: Progress,
    failure: Option<ExecutionError>,
    started: bool,
    repeated_start: bool,
}
struct Shared {
    epoch: Instant,
    state: Mutex<State>,
    wake: Arc<dyn CompletionWake>,
}

/// Observe only actual storage-span returns; unrelated completion wakes grant no extra time.
/// The unchanged stall interval guards real OS calls. The work ceiling guards productive loops.
/// Application expiry and cancellation continue to use the fixture's injected operation clock.
#[derive(Clone)]
pub(crate) struct Observation {
    shared: Arc<Shared>,
    stall: Duration,
}
impl Observation {
    /// Start one bounded native phase using its existing completion notification boundary.
    pub(crate) fn new(wake: Arc<dyn CompletionWake>, stall: Duration) -> Self {
        Self {
            shared: Arc::new(Shared {
                epoch: Instant::now(),
                state: Mutex::new(State::default()),
                wake,
            }),
            stall,
        }
    }

    /// Begin actual admitted work once, independently of time spent awaiting queue admission.
    pub(crate) fn started(&self) {
        {
            let mut state = crate::registry::lock(&self.shared.state);
            if state.started {
                state.repeated_start = true;
            } else {
                state.started = true;
                state.progress.last_return = self.shared.epoch.elapsed();
            }
        }
        self.notify();
    }

    /// Record actual storage returns without adding traffic to the completion notification port.
    pub(super) fn returned(&self) {
        {
            let mut state = crate::registry::lock(&self.shared.state);
            state.progress.returned = state.progress.returned.saturating_add(1);
            state.progress.last_return = self.shared.epoch.elapsed();
        }
    }

    fn notify(&self) {
        if let Err(cause) = self.shared.wake.wake() {
            let mut state = crate::registry::lock(&self.shared.state);
            if state.failure.is_none() {
                state.failure = Some(cause);
            }
        }
    }

    /// Return the remaining no-progress interval, preserving notification failures and work limits.
    pub(crate) fn remaining(&self) -> io::Result<Duration> {
        let progress = self.progress()?;
        remaining(self.stall, self.shared.epoch.elapsed(), progress)
    }

    /// Check finite work and delivery even when a terminal arrived before its observer ran.
    pub(crate) fn completed(&self) -> io::Result<()> {
        validate_work(self.progress()?)
    }

    pub(super) fn progress(&self) -> io::Result<Progress> {
        let mut state = crate::registry::lock(&self.shared.state);
        if let Some(cause) = state.failure.take() {
            return Err(io::Error::other(cause));
        }
        if state.repeated_start {
            return Err(io::Error::other("Native fixture observation was started twice"));
        }
        Ok(state.progress)
    }
}

/// Keep watchdog decisions pure so progress, delayed observation and stalls need no sleeps.
fn remaining(stall: Duration, now: Duration, progress: Progress) -> io::Result<Duration> {
    validate_work(progress)?;
    let idle = now
        .checked_sub(progress.last_return)
        .ok_or_else(|| io::Error::other("Native fixture progress clock regressed"))?;
    stall
        .checked_sub(idle)
        .filter(|left| !left.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Native storage made no progress"))
}

fn validate_work(progress: Progress) -> io::Result<()> {
    if progress.returned > MAX_RETURNS {
        return Err(io::Error::other("Native fixture exceeded its storage-return work limit"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_progress_can_outlast_a_total_wall_time_interval() -> io::Result<()> {
        let stall = Duration::from_secs(20);
        for second in [0, 19, 38, 57, 76] {
            let progress =
                Progress { returned: second + 1, last_return: Duration::from_secs(second) };
            assert_eq!(
                remaining(stall, Duration::from_secs(second + 1), progress)?,
                Duration::from_secs(19)
            );
        }
        Ok(())
    }

    #[test]
    fn absent_progress_expires_at_the_original_stall_limit() -> io::Result<()> {
        let stall = Duration::from_secs(20);
        assert_eq!(
            remaining(stall, Duration::from_secs(19), Progress::default())?,
            Duration::from_secs(1)
        );
        assert_eq!(
            remaining(stall, stall, Progress::default())
                .err()
                .ok_or_else(|| io::Error::other("Stalled work was accepted"))?
                .kind(),
            io::ErrorKind::TimedOut
        );
        Ok(())
    }

    #[test]
    fn late_observation_and_unrelated_wakes_cannot_renew_old_progress() {
        let progress = Progress { returned: 10, last_return: Duration::from_secs(10) };
        assert!(remaining(Duration::from_secs(20), Duration::from_secs(30), progress).is_err());
        assert!(remaining(Duration::from_secs(20), Duration::from_secs(40), progress).is_err());
    }

    #[test]
    fn continuous_activity_cannot_exceed_the_finite_work_ceiling() -> io::Result<()> {
        let now = Duration::from_secs(1000);
        assert_eq!(
            remaining(
                Duration::from_secs(20),
                now,
                Progress { returned: MAX_RETURNS, last_return: now }
            )?,
            Duration::from_secs(20)
        );
        assert!(
            remaining(
                Duration::from_secs(20),
                now,
                Progress { returned: MAX_RETURNS + 1, last_return: now }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn progress_cannot_arrive_from_a_future_observation_clock() {
        assert!(
            remaining(
                Duration::from_secs(20),
                Duration::from_secs(1),
                Progress { returned: 1, last_return: Duration::from_secs(2) }
            )
            .is_err()
        );
    }

    #[test]
    fn repeated_phase_start_cannot_renew_the_observation() {
        let observation =
            Observation::new(Arc::new(super::super::Wake::default()), Duration::from_secs(20));
        observation.started();
        observation.started();
        assert!(observation.remaining().is_err());
        assert!(observation.completed().is_err());
    }

    #[test]
    fn notification_failure_keeps_the_original_execution_cause() -> io::Result<()> {
        struct FailedWake;
        impl CompletionWake for FailedWake {
            fn wake(&self) -> Result<(), ExecutionError> {
                Err(ExecutionError::new(
                    sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
                    Box::new(io::Error::from_raw_os_error(5)),
                ))
            }
        }
        let observation = Observation::new(Arc::new(FailedWake), Duration::from_secs(20));
        observation.started();
        let error = observation
            .completed()
            .err()
            .ok_or_else(|| io::Error::other("Failed wake was accepted"))?;
        let execution = error
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<ExecutionError>())
            .ok_or_else(|| io::Error::other("Original execution cause was lost"))?;
        use std::error::Error;
        assert_eq!(
            execution
                .source()
                .and_then(|cause| cause.downcast_ref::<io::Error>())
                .and_then(io::Error::raw_os_error),
            Some(5)
        );
        Ok(())
    }
}
