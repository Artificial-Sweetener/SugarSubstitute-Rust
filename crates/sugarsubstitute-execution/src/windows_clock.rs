// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Expose fixed deadline translation through the sole execution and native clock owners.
use crate::native::NativeObservationError;
use crate::windows_lifecycle_pipe::{LifecyclePipeAdmissionError, LifecyclePipeDeadline};
use crate::{ExecutionClient, counter_deadline, windows_counter};
use sugarsubstitute_contracts::execution::{CounterDeadline, Moment};

impl ExecutionClient {
    /// Encode an original deadline from this executor's epoch into absolute QPC ticks.
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
            windows_counter::sample,
            || self.counter_moment(),
            original,
        )
    }
    /// Decode unchanged absolute QPC facts into this executor's existing epoch.
    ///
    /// The central Moment is sampled before the actual counter. Exact actual QPF
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
            windows_counter::sample,
            original,
        )
    }
    /// Encode an original lifecycle handshake deadline without renewing its budget.
    ///
    /// Uses this owner's generic QPC conversion, sampled before the central Moment.
    /// # Errors
    /// Preserves clock, expiry and arithmetic causes in `LifecyclePipeAdmissionError::Clock`.
    pub fn encode_lifecycle_deadline(
        &self,
        original: Moment,
    ) -> Result<LifecyclePipeDeadline, LifecyclePipeAdmissionError> {
        let deadline =
            self.encode_counter_deadline(original).map_err(LifecyclePipeAdmissionError::Clock)?;
        LifecyclePipeDeadline::new(deadline.ticks(), deadline.frequency()).map_err(|cause| {
            LifecyclePipeAdmissionError::Clock(NativeObservationError::new(
                "lifecycle_deadline_shape",
                cause,
            ))
        })
    }
    /// Decode an unchanged lifecycle deadline with exact native frequency and flooring.
    ///
    /// Samples this owner's central Moment before QPC; equality is already expired.
    /// # Errors
    /// Preserves clock, expiry and arithmetic causes in `LifecyclePipeAdmissionError::Clock`.
    pub fn decode_lifecycle_deadline(
        &self,
        original: LifecyclePipeDeadline,
    ) -> Result<Moment, LifecyclePipeAdmissionError> {
        let deadline =
            CounterDeadline::new(original.ticks(), original.frequency()).map_err(|cause| {
                LifecyclePipeAdmissionError::Clock(NativeObservationError::new(
                    "lifecycle_deadline_shape",
                    cause,
                ))
            })?;
        self.decode_counter_deadline(deadline).map_err(LifecyclePipeAdmissionError::Clock)
    }
    fn counter_moment(&self) -> Result<Moment, NativeObservationError> {
        self.now().map_err(|cause| NativeObservationError::new("execution_counter_clock", cause))
    }
}

#[cfg(test)]
#[path = "windows_clock_tests.rs"]
mod tests;
