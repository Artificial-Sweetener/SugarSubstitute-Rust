// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inject deterministic external clocks into the real execution owner: a regression targeted at its
//! actual callback without racing execution guardians, and manual time that moves only when advanced.
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};

mod manual;
pub use manual::ManualClock;

/// Keep callback identity private alongside the immutable injected value and actual sample count.
struct Regression {
    callback: std::thread::ThreadId,
    moment: Moment,
    samples: u64,
}
/// One constant external clock with one immutable callback-local regressed observation.
/// Other callers keep the original epoch, so the actual callback must encounter the regression
/// before execution latches global failure. This creates no thread, timer or scheduling authority.
pub struct CallbackRegressionClock {
    original: Moment,
    regression: Mutex<Option<Regression>>,
}
impl CallbackRegressionClock {
    /// Establish the original injected epoch before constructing the actual execution owner.
    pub fn new(original: Moment) -> Arc<Self> {
        Arc::new(Self { original, regression: Mutex::new(None) })
    }
    /// Arm the current synchronous external callback once, without sampling execution time.
    /// Returns false for an already armed fixture or a moment that does not precede the epoch.
    /// No callback identity or scheduler handle is exposed to consumers.
    pub fn arm_current_callback(&self, regressed: Moment) -> bool {
        let mut slot = self.regression.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_some() || regressed >= self.original {
            return false;
        }
        *slot = Some(Regression {
            callback: std::thread::current().id(),
            moment: regressed,
            samples: 0,
        });
        true
    }
    /// Observe only the immutable requested test value, excluding actual clock consumption.
    pub fn requested_regression(&self) -> Option<Moment> {
        self.regression
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|regression| regression.moment)
    }
    /// Count actual provider observations of the targeted regressed value.
    /// Execution's own fatal-clock latch remains authoritative for failure and shutdown.
    pub fn regressed_samples(&self) -> u64 {
        self.regression
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map_or(0, |regression| regression.samples)
    }
}
impl Clock for CallbackRegressionClock {
    fn now(&self) -> Moment {
        let mut slot = self.regression.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(regression) = &mut *slot
            && regression.callback == std::thread::current().id()
        {
            regression.samples = regression.samples.saturating_add(1);
            return regression.moment;
        }
        self.original
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[cfg(test)]
mod tests;
