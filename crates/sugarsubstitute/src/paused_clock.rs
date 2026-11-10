// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Freeze logical time for tests whose product code takes real deadlines.
//!
//! Product services bound their work with deadlines on an injected clock. A test that gives them
//! this clock observes a deadline that never expires, however slowly a loaded machine runs, while
//! the service's own polling timers still fire after their requested durations. Time is therefore
//! decided by the test and elapsed time only paces polling; it never changes an outcome.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};
use sugarsubstitute_execution::MonotonicClock;

/// A clock whose `now` never moves and whose timers wait out the requested duration for real.
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

    /// Wait for the logical distance to `deadline` on the real timer owned by the execution adapter.
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        let remaining = deadline.checked_duration_since(self.frozen).unwrap_or(Duration::ZERO);
        let wait: Pin<Box<dyn Future<Output = ()> + Send>> =
            match self.real.now().checked_add(remaining) {
                Ok(real_deadline) => self.real.sleep_until(real_deadline),
                Err(_) => Box::pin(std::future::pending()),
            };
        wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion_wake::Wake;
    use sugarsubstitute_contracts::execution::{
        ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Operation, ScopeId, ScopeSpec,
    };
    use sugarsubstitute_execution::ExecutionOwner;

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
