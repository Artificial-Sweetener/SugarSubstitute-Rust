// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve actual retirement metadata after a fatal public clock observation.

use crate::{scope::lock, task};
use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, Weak},
};
use sugarsubstitute_contracts::execution::{Clock, ExecutionError, Moment, TimeError, WaitFuture};

#[derive(Debug)]
struct Failure {
    message: String,
    time: Option<TimeError>,
}
impl Failure {
    fn from_payload(payload: &(dyn std::any::Any + Send)) -> Self {
        let time = payload.downcast_ref::<TimeError>().copied();
        Self {
            message: time.map_or_else(
                || task::borrowed_panic_cause(payload).as_str().to_owned(),
                |cause| cause.to_string(),
            ),
            time,
        }
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.time.as_ref().map(|cause| cause as &dyn std::error::Error)
    }
}

struct State {
    last: Moment,
    failure: Option<Arc<Failure>>,
    notified: bool,
}

pub(crate) struct RetirementClock {
    supplied: Arc<dyn Clock>,
    owner: Weak<crate::admission::Core>,
    state: Arc<Mutex<State>>,
}
impl RetirementClock {
    /// Establish a real epoch observation before any runtime or worker exists.
    pub(crate) fn new(supplied: Arc<dyn Clock>, owner: Weak<crate::admission::Core>) -> Self {
        let last = supplied.now();
        Self {
            supplied,
            owner,
            state: Arc::new(Mutex::new(State { last, failure: None, notified: false })),
        }
    }

    pub(crate) fn failed(&self) -> bool {
        lock(&self.state).failure.is_some()
    }

    pub(crate) fn now(&self) -> Moment {
        Clock::now(self)
    }

    pub(crate) fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        Clock::sleep_until(self, deadline)
    }

    fn latch(&self, payload: &(dyn std::any::Any + Send)) {
        Self::latch_state(&self.state, &self.owner, payload);
    }
    fn latch_state(
        state: &Mutex<State>,
        owner: &Weak<crate::admission::Core>,
        payload: &(dyn std::any::Any + Send),
    ) {
        let notice = {
            let mut state = lock(state);
            if state.failure.is_none() {
                state.failure = Some(Arc::new(Failure::from_payload(payload)));
            }
            if state.notified {
                None
            } else {
                state.notified = true;
                state.failure.as_ref().map(|cause| crate::runtime::error(cause.clone()))
            }
        };
        if let Some(owner) = owner.upgrade() {
            owner.shutdown();
            if let Some(notice) = notice {
                owner.mailbox.record_failure(notice);
                owner.mailbox.deliver_wake();
            }
        }
    }

    fn discard_payload(&self, payload: Box<dyn std::any::Any + Send>) {
        if payload.is::<TimeError>() {
            // This fixed typed clock failure is already retained by latch_state.
            return;
        }
        let observed = task::panic_cause(payload);
        let changed = lock(&self.state)
            .failure
            .as_ref()
            .is_some_and(|cause| cause.message != observed.as_str());
        if changed && let Some(owner) = self.owner.upgrade() {
            owner.mailbox.record_failure(crate::runtime::error(std::io::Error::other(
                observed.as_str().to_owned(),
            )));
        }
    }

    /// Pre-dispatch/shutdown guardians preserve worker ownership after first failure.
    pub(crate) fn retirement_now(&self) -> Moment {
        match catch_unwind(AssertUnwindSafe(|| self.now())) {
            Ok(moment) => moment,
            Err(payload) => {
                self.discard_payload(payload);
                lock(&self.state).last
            }
        }
    }

    pub(crate) fn retirement_sleep_until(&self, deadline: Moment) -> WaitFuture {
        match catch_unwind(AssertUnwindSafe(|| self.sleep_until(deadline))) {
            Ok(wait) => wait,
            Err(payload) => {
                self.discard_payload(payload);
                Box::pin(std::future::pending())
            }
        }
    }

    /// Public metadata never exposes a cached retirement timestamp as fresh time.
    pub(crate) fn metadata(&self) -> Result<Moment, ExecutionError> {
        if let Some(failure) = &lock(&self.state).failure {
            return Err(crate::runtime::error(failure.clone()));
        }
        match catch_unwind(AssertUnwindSafe(|| self.now())) {
            Ok(moment) => {
                let state = lock(&self.state);
                match &state.failure {
                    Some(failure) => Err(crate::runtime::error(failure.clone())),
                    None => Ok(moment),
                }
            }
            Err(payload) => {
                self.discard_payload(payload);
                let state = lock(&self.state);
                match &state.failure {
                    Some(failure) => Err(crate::runtime::error(failure.clone())),
                    None => Err(crate::runtime::error(std::io::Error::other(
                        "Clock failed without retained cause",
                    ))),
                }
            }
        }
    }
}
impl RetirementClock {
    /// Sample without calling the owner while a native issue fence is held.
    /// The caller must release its locks before latching a returned panic payload.
    pub(crate) fn observe_unlatched(&self) -> Result<Moment, Box<dyn std::any::Any + Send>> {
        let completed_before_sample = {
            let state = lock(&self.state);
            if state.failure.is_some() {
                return Ok(state.last);
            }
            state.last
        };
        let observed = catch_unwind(AssertUnwindSafe(|| self.supplied.now()))?;
        let mut state = lock(&self.state);
        if state.failure.is_none() {
            if observed < completed_before_sample {
                // Fence concurrent admission immediately. Owner shutdown and wake
                // delivery remain deferred until the caller releases its issue fence.
                state.failure = Some(Arc::new(Failure::from_payload(&TimeError::Regression)));
                return Err(Box::new(TimeError::Regression));
            }
            // Concurrent provider calls may finish out of sampling order. Only
            // completions preceding this call bound its result; publishing the
            // completed maximum keeps every consumer's remaining budget conservative.
            state.last = state.last.max(observed);
            Ok(state.last)
        } else {
            Ok(state.last)
        }
    }
    /// Preserve and resume a failed sample after the caller releases its issue fence.
    pub(crate) fn fail_observation(&self, payload: Box<dyn std::any::Any + Send>) -> ! {
        self.latch(payload.as_ref());
        std::panic::resume_unwind(payload)
    }
}

#[cfg(test)]
#[path = "clock_retirement_tests.rs"]
mod tests;
impl Clock for RetirementClock {
    fn now(&self) -> Moment {
        match self.observe_unlatched() {
            Ok(observed) => observed,
            Err(payload) => self.fail_observation(payload),
        }
    }

    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        if self.failed() {
            // Failure cannot fabricate elapsed time or immediate timer progress.
            Box::pin(std::future::pending())
        } else {
            match catch_unwind(AssertUnwindSafe(|| self.supplied.sleep_until(deadline))) {
                Ok(mut wait) => {
                    let state = self.state.clone();
                    let owner = self.owner.clone();
                    Box::pin(std::future::poll_fn(move |context| {
                        if lock(&state).failure.is_some() {
                            return std::task::Poll::Pending;
                        }
                        match catch_unwind(AssertUnwindSafe(|| wait.as_mut().poll(context))) {
                            Ok(observed) => observed,
                            Err(payload) => {
                                Self::latch_state(&state, &owner, payload.as_ref());
                                std::panic::resume_unwind(payload)
                            }
                        }
                    }))
                }
                Err(payload) => {
                    self.latch(payload.as_ref());
                    std::panic::resume_unwind(payload)
                }
            }
        }
    }
}
