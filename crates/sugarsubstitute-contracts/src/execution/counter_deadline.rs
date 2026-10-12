// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Carry inert cross-process counter facts without native or scheduling authority.
use std::{error::Error, fmt};

/// Reject counter facts that cannot describe a positive signed native counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CounterDeadlineError {
    /// Absolute ticks and ticks per second must both be positive.
    NonPositive,
}
impl fmt::Display for CounterDeadlineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Counter deadline ticks and frequency must be positive")
    }
}
impl Error for CounterDeadlineError {}

/// Immutable absolute counter deadline and its enrolled ticks-per-second frequency.
///
/// These facts grant no clock, process, timer or authentication authority. The
/// receiving execution owner verifies its actual counter frequency and translates
/// only the remaining interval into its existing local clock epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CounterDeadline {
    ticks: i64,
    frequency: i64,
}
impl CounterDeadline {
    /// Construct positive signed absolute ticks and ticks per second.
    ///
    /// # Errors
    /// Returns `CounterDeadlineError::NonPositive` for either nonpositive value.
    pub const fn new(ticks: i64, frequency: i64) -> Result<Self, CounterDeadlineError> {
        if ticks <= 0 || frequency <= 0 {
            return Err(CounterDeadlineError::NonPositive);
        }
        Ok(Self { ticks, frequency })
    }
    /// Return absolute ticks in the enrolled counter epoch.
    pub const fn ticks(self) -> i64 {
        self.ticks
    }
    /// Return the enrolled counter frequency in ticks per second.
    pub const fn frequency(self) -> i64 {
        self.frequency
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn facts_preserve_signed_counter_units() -> Result<(), CounterDeadlineError> {
        let deadline = CounterDeadline::new(i64::MAX, 3)?;
        assert_eq!(deadline.ticks(), i64::MAX);
        assert_eq!(deadline.frequency(), 3);
        Ok(())
    }
    #[test]
    fn facts_reject_every_nonpositive_component() {
        for (ticks, frequency) in [(0, 1), (1, 0), (-1, 1), (1, -1), (i64::MIN, 1), (1, i64::MIN)] {
            assert_eq!(
                CounterDeadline::new(ticks, frequency),
                Err(CounterDeadlineError::NonPositive)
            );
        }
    }
}
