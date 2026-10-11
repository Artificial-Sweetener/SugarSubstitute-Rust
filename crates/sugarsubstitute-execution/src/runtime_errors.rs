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
