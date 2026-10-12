// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Freeze one original monotonic operation interval independently of UTC facts.
use std::time::Duration;
use sugarsubstitute_contracts::{
    execution::{Moment, TimeError},
    incident::IncidentUtc,
};

/// One immutable sixty-second interval in the caller's injected execution clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidentOperationBudget {
    started: Moment,
    until: Moment,
}
impl IncidentOperationBudget {
    /// Freeze exactly sixty seconds once; overflowing monotonic arithmetic rejects.
    pub fn new(started: Moment) -> Result<Self, TimeError> {
        Ok(Self { started, until: started.checked_add(Duration::from_secs(60))? })
    }
    /// Return the original start in monotonic nanoseconds.
    pub const fn started(self) -> Moment {
        self.started
    }
    /// Return the fixed exclusive completion deadline; retries cannot renew it.
    pub const fn until(self) -> Moment {
        self.until
    }
}
/// Retain the request's original interval and separately injected UTC timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidentRequestContext {
    budget: IncidentOperationBudget,
    at: IncidentUtc,
}
impl IncidentRequestContext {
    /// Combine independent monotonic and UTC values without reading either clock.
    pub const fn new(budget: IncidentOperationBudget, at: IncidentUtc) -> Self {
        Self { budget, at }
    }
    /// Return the unchanged interval.
    pub const fn budget(self) -> IncidentOperationBudget {
        self.budget
    }
    /// Return the actual request's UTC timestamp.
    pub const fn at(self) -> IncidentUtc {
        self.at
    }
}
