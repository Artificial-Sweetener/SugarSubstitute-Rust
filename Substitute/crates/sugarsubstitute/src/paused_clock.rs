// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Freeze logical time for tests whose product code takes real deadlines.
//!
//! Product services bound their work with deadlines on an injected clock. A test that gives them
//! this clock observes a deadline that never expires, however slowly a loaded machine runs. The
//! service's own polling delays still elapse for real, so time is decided by the test and elapsed
//! time only paces polling; it never changes an outcome.
//!
//! The clock port has one timer for both purposes, so the distance to the instant tells them
//! apart: a wait up to [`DELAY_LIMIT`] is a polling delay, and a longer one is a deadline that
//! logical time, being frozen, never reaches.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};
use sugarsubstitute_execution::MonotonicClock;

/// Longest wait treated as a polling delay. Execution owners arm job deadlines as timers on the
/// same port, and those must not elapse in real time on a slow machine.
const DELAY_LIMIT: Duration = Duration::from_secs(1);

/// A clock whose `now` never moves, whose short timers wait out the requested duration for real
/// and whose long timers, the deadlines, never fire.
pub(crate) struct PausedClock {
    real: Arc<MonotonicClock>,
    frozen: Moment,
}

impl PausedClock {
    /// Freeze logical time at zero.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self { real: MonotonicClock::new(), frozen: Moment::from_nanos(0) })
    }
}

impl Clock for PausedClock {
    fn now(&self) -> Moment {
        self.frozen
    }

    /// Wait out a polling delay on the real timer owned by the execution adapter; a deadline
    /// beyond [`DELAY_LIMIT`] stays pending forever.
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        let remaining = deadline.checked_duration_since(self.frozen).unwrap_or(Duration::ZERO);
        let wait: Pin<Box<dyn Future<Output = ()> + Send>> = match polling_delay(remaining)
            .and_then(|delay| self.real.now().checked_add(delay).ok())
        {
            Some(real_deadline) => self.real.sleep_until(real_deadline),
            None => Box::pin(std::future::pending()),
        };
        wait
    }
}

/// The real wait for a timer `remaining` logical time away, or none for a deadline.
fn polling_delay(remaining: Duration) -> Option<Duration> {
    (remaining <= DELAY_LIMIT).then_some(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion_wake::Wake;
    use sugarsubstitute_contracts::execution::{
        ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Operation, ScopeId, ScopeSpec,
    };
    use sugarsubstitute_execution::ExecutionOwner;

    /// Polling delays elapse for real while a deadline, however near in logical time, never does.
    #[test]
    fn only_waits_up_to_the_delay_limit_are_real_timers() {
        assert_eq!(polling_delay(Duration::ZERO), Some(Duration::ZERO));
        assert_eq!(polling_delay(Duration::from_millis(1)), Some(Duration::from_millis(1)));
        assert_eq!(polling_delay(DELAY_LIMIT), Some(DELAY_LIMIT));
        assert_eq!(polling_delay(DELAY_LIMIT + Duration::from_nanos(1)), None);
        assert_eq!(polling_delay(Duration::from_secs(60)), None);
    }

    /// A timer fires after its requested duration while logical time stays where the test put it.
    #[test]
    fn timers_fire_for_real_while_logical_time_stays_frozen()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = PausedClock::new();
        let wake = Arc::new(Wake::default());
        let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::IoOrdinary,
            deadline: Some(Moment::from_nanos(1_000_000_000)),
            reservation_bytes: 1024,
            replacement: None,
        };
        let mut job = client.submit_async(&scope, spec, |context| async move {
            let before = context.now();
            context.sleep_until(before.checked_add(Duration::from_millis(20))?).await;
            Ok::<_, sugarsubstitute_contracts::execution::TimeError>((before, context.now()))
        })?;
        let JobOutcome::Success((before, after)) = wake.completion(&client, &mut job)?.outcome
        else {
            return Err("The timer job did not complete".into());
        };
        assert_eq!(before, Moment::from_nanos(0));
        assert_eq!(after, before);
        drop((job, scope));
        wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(1))?;
        Ok(())
    }
}
