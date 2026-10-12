// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve one original setup ancestor across every bounded discovery probe.

use super::EntryBudgetError;
use crate::execution::{Moment, TimeError};
use std::time::Duration;

/// Original thirty-second setup budget in the execution owner's monotonic clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntrySetupBudget {
    started: Moment,
    until: Moment,
}
impl EntrySetupBudget {
    /// Freeze one checked thirty-second ancestor before queue admission.
    /// # Errors
    /// Returns the original clock overflow if the ancestor is unrepresentable.
    pub fn new(started: Moment) -> Result<Self, TimeError> {
        Ok(Self { started, until: started.checked_add(Duration::from_secs(30))? })
    }
    /// Return the original monotonic setup start.
    pub const fn started(self) -> Moment {
        self.started
    }
    /// Return the unchanged original setup deadline.
    pub const fn until(self) -> Moment {
        self.until
    }
    /// Bound a probe to two seconds and the remaining original ancestor.
    /// # Errors
    /// Rejects a regressed start, expiry including equality, or checked clock failure.
    /// Capping before addition preserves valid short probes near the clock limit.
    pub fn probe_until(self, attempt_started: Moment) -> Result<Moment, EntryBudgetError> {
        if attempt_started < self.started {
            return Err(EntryBudgetError::Clock(TimeError::Regression));
        }
        if attempt_started >= self.until {
            return Err(EntryBudgetError::Expired);
        }
        let remaining =
            self.until.checked_duration_since(attempt_started).map_err(EntryBudgetError::Clock)?;
        attempt_started
            .checked_add(remaining.min(Duration::from_secs(2)))
            .map_err(EntryBudgetError::Clock)
    }
}
