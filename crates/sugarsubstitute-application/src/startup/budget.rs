// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Freeze startup deadlines in the composing owner's injected execution-clock epoch.
//!
//! These values schedule no work and grant no connection or frontend authority.
//! The host creates each budget once, then retains it across retries and duplicate
//! grants. Every supplied moment must belong to that same monotonic clock epoch.

use std::{error::Error, fmt, time::Duration};
use sugarsubstitute_contracts::execution::{Moment, TimeError};

/// Select the original preparation duration for the configured connection ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionBudgetKind {
    /// Prepare an attached connection within one hundred fifty seconds without process control.
    Attached,
    /// Prepare a managed backend within one hour.
    Managed,
}

/// Reject a probe start without changing the original campaign budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeBudgetError {
    /// Preserve the original checked monotonic arithmetic cause.
    Clock(TimeError),
    /// The attempt starts at or after the original campaign deadline.
    Expired,
    /// Supplied remaining interval exceeds the configured original role bound.
    IntervalTooLong,
}

impl fmt::Display for ProbeBudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(cause) => cause.fmt(formatter),
            Self::Expired => formatter.write_str("Original connection campaign deadline elapsed"),
            Self::IntervalTooLong => {
                formatter.write_str("Connection interval exceeds its role bound")
            }
        }
    }
}

impl Error for ProbeBudgetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(cause) => Some(cause),
            Self::Expired | Self::IntervalTooLong => None,
        }
    }
}

/// Retain one original connection preparation deadline across every probe attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectionBudget {
    kind: ConnectionBudgetKind,
    started: Moment,
    until: Moment,
}

impl ConnectionBudget {
    /// Freeze the attached one-hundred-fifty-second or managed one-hour preparation budget.
    ///
    /// `started` uses the composing owner's monotonic nanosecond epoch. Construct
    /// this value before the first preparation operation and retain it across retries.
    ///
    /// # Errors
    /// Returns `TimeError::Overflow` if the original deadline is unrepresentable.
    pub fn new(kind: ConnectionBudgetKind, started: Moment) -> Result<Self, TimeError> {
        let duration = match kind {
            ConnectionBudgetKind::Attached => Duration::from_secs(150),
            ConnectionBudgetKind::Managed => Duration::from_secs(3600),
        };
        Ok(Self { kind, started, until: started.checked_add(duration)? })
    }

    /// Retain an already validated original cutoff in the composing clock epoch.
    /// `observed` is the actual local receive time; `until` stays byte-for-byte
    /// equivalent as a Moment across later dispatch and probe retries. This pure
    /// data constructor authenticates no peer and grants no native authority.
    ///
    /// # Errors
    /// Rejects equality/expiry and remaining intervals beyond the attached
    /// 150-second or managed one-hour limit. Checked clock causes remain typed.
    pub fn from_until(
        kind: ConnectionBudgetKind,
        observed: Moment,
        until: Moment,
    ) -> Result<Self, ProbeBudgetError> {
        if observed >= until {
            return Err(ProbeBudgetError::Expired);
        }
        let remaining = until.checked_duration_since(observed).map_err(ProbeBudgetError::Clock)?;
        let maximum = match kind {
            ConnectionBudgetKind::Attached => Duration::from_secs(150),
            ConnectionBudgetKind::Managed => Duration::from_secs(3600),
        };
        if remaining > maximum {
            return Err(ProbeBudgetError::IntervalTooLong);
        }
        Ok(Self { kind, started: observed, until })
    }

    /// Return the connection ownership that selected the original duration.
    pub const fn kind(&self) -> ConnectionBudgetKind {
        self.kind
    }

    /// Return the original start in the composing clock's monotonic nanosecond epoch.
    pub const fn started(&self) -> Moment {
        self.started
    }

    /// Return the immutable campaign deadline in that same clock epoch.
    pub const fn until(&self) -> Moment {
        self.until
    }

    /// Bound one admitted readiness probe by two seconds without an aggregate startup cutoff.
    ///
    /// This arithmetic grants no ownership or wait admission. The campaign first
    /// binds an actual cancellable scope after finite preparation; managed callers
    /// additionally retain their actual process exit watch. The original budget is unchanged.
    ///
    /// # Errors
    /// Rejects regression before the original start and checked
    /// addition overflow. Elapsed preparation time alone does not reject a live probe.
    pub fn readiness_probe_until(
        &self,
        attempt_started: Moment,
    ) -> Result<Moment, ProbeBudgetError> {
        if attempt_started < self.started {
            return Err(ProbeBudgetError::Clock(TimeError::Regression));
        }
        attempt_started.checked_add(Duration::from_secs(2)).map_err(ProbeBudgetError::Clock)
    }

    /// Bound one probe by two seconds and the remaining original campaign interval.
    ///
    /// The attempt must start within `[started, until)` in the same injected epoch.
    /// Repeated calls neither consume nor renew the campaign budget. Capping the
    /// interval before checked addition permits valid probes near the clock limit.
    ///
    /// # Errors
    /// Returns `ProbeBudgetError::Clock(TimeError::Regression)` for a start preceding
    /// the original start, or `ProbeBudgetError::Expired` at or beyond the original
    /// deadline. Other checked arithmetic errors retain their typed clock cause.
    pub fn probe_until(&self, attempt_started: Moment) -> Result<Moment, ProbeBudgetError> {
        if attempt_started < self.started {
            return Err(ProbeBudgetError::Clock(TimeError::Regression));
        }
        if attempt_started >= self.until {
            return Err(ProbeBudgetError::Expired);
        }
        let remaining =
            self.until.checked_duration_since(attempt_started).map_err(ProbeBudgetError::Clock)?;
        attempt_started
            .checked_add(remaining.min(Duration::from_secs(2)))
            .map_err(ProbeBudgetError::Clock)
    }
}

/// Retain the original one-hour native Main preparation deadline.
///
/// The host freezes this local budget before constructing the Main request. Folder,
/// environment and browser authorization retain its cutoff until native authorization.
/// Frontend readiness follows its actual loading lifecycle independently of this budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontendBudget {
    started: Moment,
    until: Moment,
}

impl FrontendBudget {
    /// Freeze one native Main preparation deadline in the composing clock's epoch.
    ///
    /// # Errors
    /// Returns `TimeError::Overflow` if the original deadline is unrepresentable.
    pub fn new(started: Moment) -> Result<Self, TimeError> {
        Ok(Self { started, until: started.checked_add(Duration::from_secs(3600))? })
    }

    /// Return the original start in the composing clock's monotonic nanosecond epoch.
    pub const fn started(&self) -> Moment {
        self.started
    }

    /// Return the immutable one-hour deadline in that same clock epoch.
    pub const fn until(&self) -> Moment {
        self.until
    }
}

#[cfg(test)]
mod tests;
