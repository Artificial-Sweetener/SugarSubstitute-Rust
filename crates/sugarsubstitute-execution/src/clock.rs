// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Supply monotonic observations and owned waits to the private execution runtime.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, TimeError, WaitFuture};

/// Observe elapsed monotonic time from one executor-local epoch.
///
/// Wait futures are polled only by the execution adapter's private timer runtime.
pub struct MonotonicClock {
    epoch: Instant,
}

impl MonotonicClock {
    /// Create a clock whose zero epoch is this construction instant.
    pub fn new() -> Arc<Self> {
        Arc::new(Self { epoch: Instant::now() })
    }
    /// Map a Moment from this clock's epoch to its actual monotonic Instant.
    ///
    /// Presentation consumers can retain the same original deadline for native
    /// waits. This query creates no timer and accepts elapsed moments, including
    /// zero; the wait owner determines whether a deadline has already expired.
    ///
    /// # Errors
    /// Returns `TimeError::Overflow` when the platform cannot represent the Instant.
    pub fn instant_for(&self, moment: Moment) -> Result<Instant, TimeError> {
        self.epoch.checked_add(Duration::from_nanos(moment.as_nanos())).ok_or(TimeError::Overflow)
    }
}

impl Clock for MonotonicClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX))
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        let target = self.instant_for(deadline);
        Box::pin(async move {
            match target {
                Ok(target) => {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(target)).await
                }
                Err(_) => std::future::pending().await,
            }
        })
    }
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
