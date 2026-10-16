// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain every bounded worker teardown cause.

use sugarsubstitute_contracts::execution::ExecutionError;

#[derive(Debug)]
struct TeardownCauses(Vec<ExecutionError>);
impl std::fmt::Display for TeardownCauses {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} owned execution teardown failures", self.0.len())?;
        for cause in &self.0 {
            write!(formatter, "; {cause}")?;
        }
        Ok(())
    }
}
impl std::error::Error for TeardownCauses {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.first().map(|cause| cause as &(dyn std::error::Error + 'static))
    }
}
pub(crate) fn aggregate_errors(causes: Vec<ExecutionError>) -> ExecutionError {
    let operation = causes.first().map_or(
        sugarsubstitute_contracts::execution::Operation::Infrastructure,
        ExecutionError::operation,
    );
    ExecutionError::new(operation, Box::new(TeardownCauses(causes)))
}

#[cfg(any(windows, test))]
impl crate::native::NativeObservationError {
    /// Borrow the original native failure through execution-owned error envelopes.
    ///
    /// Only concrete `ExecutionError` boxes and this adapter's teardown aggregate
    /// are followed. An aggregate's first cause remains authoritative even when
    /// opaque; later failures never replace it. The owned acyclic envelopes are
    /// traversed with constant stack usage, without allocation, formatting, native
    /// effects, or arbitrary `Error::source` callbacks. Custody remains unchanged.
    pub fn from_execution_cause<'a>(
        mut cause: &'a (dyn std::error::Error + 'static),
    ) -> Option<&'a Self> {
        loop {
            if let Some(native) = cause.downcast_ref::<Self>() {
                return Some(native);
            }
            if let Some(execution) = cause.downcast_ref::<ExecutionError>() {
                cause = <ExecutionError as std::error::Error>::source(execution)?;
            } else if let Some(aggregate) = cause.downcast_ref::<TeardownCauses>() {
                cause = aggregate.0.first()?;
            } else {
                return None;
            }
        }
    }
}
