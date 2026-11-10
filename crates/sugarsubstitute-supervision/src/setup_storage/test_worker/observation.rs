// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound native qualification by returned work and delivery, never by elapsed wall-clock time.
use std::{
    io,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};

/// Cover the largest 8192-entry fixture with at most 32 nested storage returns per entry.
/// Repeated activity cannot keep an erroneous native loop alive without a finite work ceiling.
const MAX_RETURNS: u64 = 8192 * 32;

#[derive(Clone, Copy, Default)]
pub(super) struct Progress {
    pub(super) returned: u64,
}
#[derive(Default)]
struct State {
    progress: Progress,
    failure: Option<ExecutionError>,
    started: bool,
    repeated_start: bool,
}
struct Shared {
    state: Mutex<State>,
    wake: Arc<dyn CompletionWake>,
}

/// Observe only actual storage-span returns; unrelated completion wakes grant no extra work.
/// The work ceiling guards productive loops. A native call that never returns leaves its test
/// running, which the runner reports, because no wall-clock interval can tell a stalled call from
/// storage that is merely slow on a loaded machine.
/// Application expiry and cancellation continue to use the fixture's injected operation clock.
#[derive(Clone)]
pub(crate) struct Observation {
    shared: Arc<Shared>,
}
impl Observation {
    /// Start one bounded native phase using its existing completion notification boundary.
    pub(crate) fn new(wake: Arc<dyn CompletionWake>) -> Self {
        Self { shared: Arc::new(Shared { state: Mutex::new(State::default()), wake }) }
    }

    /// Begin actual admitted work once, independently of time spent awaiting queue admission.
    pub(crate) fn started(&self) {
        {
            let mut state = crate::registry::lock(&self.shared.state);
            if state.started {
                state.repeated_start = true;
            } else {
                state.started = true;
            }
        }
        self.notify();
    }

    /// Record actual storage returns without adding traffic to the completion notification port.
    pub(super) fn returned(&self) {
        let mut state = crate::registry::lock(&self.shared.state);
        state.progress.returned = state.progress.returned.saturating_add(1);
    }

    fn notify(&self) {
        if let Err(cause) = self.shared.wake.wake() {
            let mut state = crate::registry::lock(&self.shared.state);
            if state.failure.is_none() {
                state.failure = Some(cause);
            }
        }
    }

    /// Reject a lost notification, a repeated start or exhausted finite work while work continues.
    pub(crate) fn check(&self) -> io::Result<()> {
        validate_work(self.progress()?)
    }

    /// Check finite work and delivery even when a terminal arrived before its observer ran.
    pub(crate) fn completed(&self) -> io::Result<()> {
        self.check()
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
    fn continuous_activity_cannot_exceed_the_finite_work_ceiling() {
        assert!(validate_work(Progress { returned: MAX_RETURNS }).is_ok());
        assert!(validate_work(Progress { returned: MAX_RETURNS + 1 }).is_err());
    }

    #[test]
    fn returns_accumulate_until_the_ceiling_rejects_the_observation() {
        let observation = Observation::new(Arc::new(super::super::Wake::default()));
        observation.started();
        for _ in 0..MAX_RETURNS {
            observation.returned();
        }
        assert!(observation.check().is_ok());
        observation.returned();
        assert!(observation.check().is_err());
        assert!(observation.completed().is_err());
    }

    #[test]
    fn repeated_phase_start_cannot_renew_the_observation() {
        let observation = Observation::new(Arc::new(super::super::Wake::default()));
        observation.started();
        observation.started();
        assert!(observation.check().is_err());
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
        let observation = Observation::new(Arc::new(FailedWake));
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
