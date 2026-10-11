// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Supply monotonic observations and owned waits to the private execution runtime.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};

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
}

impl Clock for MonotonicClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX))
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        let target = self.epoch.checked_add(Duration::from_nanos(deadline.as_nanos()));
        Box::pin(async move {
            match target {
                Some(target) => {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(target)).await
                }
                None => std::future::pending().await,
            }
        })
    }
}
