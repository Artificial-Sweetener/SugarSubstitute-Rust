// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Translate lifecycle pipe deadlines through the execution owner's counter conversions.
use crate::ExecutionClient;
use crate::native::NativeObservationError;
use crate::windows_lifecycle_pipe::{LifecyclePipeAdmissionError, LifecyclePipeDeadline};
use sugarsubstitute_contracts::execution::{CounterDeadline, Moment};

impl ExecutionClient {
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
}
