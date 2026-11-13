// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Translate deadlines between the execution clock and the system-wide monotonic counter.
//!
//! Native peers in other processes read the same counter: QPC on Windows and
//! `CLOCK_MONOTONIC` on Unix, so an encoded deadline means the same instant to both.
use crate::native::NativeObservationError;
use crate::{ExecutionClient, counter_deadline, platform_counter};
use sugarsubstitute_contracts::execution::{CounterDeadline, Moment};

impl ExecutionClient {
    /// Decode a diagnostic cutoff, returning `None` when it already expired.
    /// This grants no native admission or permission to renew an operation.
    /// # Errors
    /// Preserves native clock, frequency and arithmetic errors.
    pub fn decode_live_counter_deadline(
        &self,
        original: CounterDeadline,
    ) -> Result<Option<Moment>, NativeObservationError> {
        counter_deadline::decode_if_live(
            self.counter_moment()?,
            platform_counter::sample()?,
            original,
        )
    }
    /// Encode an original deadline from this executor's epoch into absolute system counter ticks.
    ///
    /// The actual native counter is sampled before this owner's central Moment.
    /// Checked integer flooring never extends the remaining original interval.
    /// This finite query creates no wait, task, process or new clock epoch.
    ///
    /// # Errors
    /// Preserves actual native counter and central clock causes; rejects expired,
    /// sub-tick or overflowing input. A central clock failure closes admission.
    pub fn encode_counter_deadline(
        &self,
        original: Moment,
    ) -> Result<CounterDeadline, NativeObservationError> {
        counter_deadline::encode_sampled(
            platform_counter::sample,
            || self.counter_moment(),
            original,
        )
    }
    /// Decode unchanged absolute system counter facts into this executor's existing epoch.
    ///
    /// The central Moment is sampled before the actual counter. Exact actual counter
    /// frequency and checked integer flooring conservatively retain only the
    /// remaining interval; receiving these inert facts grants no native authority.
    ///
    /// # Errors
    /// Preserves native and central clock causes and rejects frequency mismatch,
    /// expired, sub-nanosecond or overflowing input. Clock failure closes admission.
    pub fn decode_counter_deadline(
        &self,
        original: CounterDeadline,
    ) -> Result<Moment, NativeObservationError> {
        counter_deadline::decode_sampled(
            || self.counter_moment(),
            platform_counter::sample,
            original,
        )
    }
    pub(crate) fn counter_moment(&self) -> Result<Moment, NativeObservationError> {
        self.now().map_err(|cause| NativeObservationError::new("execution_counter_clock", cause))
    }
}

#[cfg(test)]
#[path = "counter_clock_tests.rs"]
mod tests;
