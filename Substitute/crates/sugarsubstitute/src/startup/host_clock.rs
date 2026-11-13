// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Give the startup host one clock for execution time and native event-loop deadlines.
use std::time::Instant;
use sugarsubstitute_contracts::execution::{Clock, Moment, TimeError};
use sugarsubstitute_execution::MonotonicClock;

/// The startup host's monotonic clock: the execution time every owner and job deadline reads,
/// plus the native `Instant` the desktop event loop waits for at the host's next deadline.
///
/// Production uses [`MonotonicClock`]. A test host can supply a clock that never advances on
/// its own, so no deadline expires because the machine running the test is slow.
pub trait HostClock: Clock {
    /// Map a moment of this clock to the native instant the event loop waits for.
    /// # Errors
    /// Returns `TimeError::Overflow` when the platform cannot represent the instant.
    fn instant_for(&self, moment: Moment) -> Result<Instant, TimeError>;
}

impl HostClock for MonotonicClock {
    fn instant_for(&self, moment: Moment) -> Result<Instant, TimeError> {
        Self::instant_for(self, moment)
    }
}
