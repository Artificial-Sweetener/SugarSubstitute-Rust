// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Translate native output observations without changing backend readiness or ownership.
use super::{AdmittedOutputBatch, CoordinatorBackend, ResourceKey, Result};
use sugarsubstitute_execution::windows_output::WindowsOutputError;

impl CoordinatorBackend {
    /// Observe the exact resource's delivery-slot availability without taking bytes.
    /// This grants no output permission; take_output repeats native admission.
    pub(crate) fn output_loan_available(&self, key: ResourceKey) -> Result<bool> {
        let record = self.record(key)?;
        Ok(record.output.as_ref().is_none_or(|output| output.control.loan_available()))
    }

    /// Transfer one charged output guard from the exact retained backend generation.
    /// An admitted worker awaiting construction has no data yet. Its original job,
    /// reservation and terminal observation stay with the backend owner.
    /// # Errors
    /// Rejects foreign or stale resource generations before touching output. Preserves
    /// every native rejection except the pending construction observation.
    pub(crate) fn take_output(&self, key: ResourceKey) -> Result<Option<AdmittedOutputBatch>> {
        let record = self.record(key)?;
        match &record.output {
            Some(output) => match output.control.take_batch() {
                Err(WindowsOutputError::NotReady) => Ok(None),
                observation => Ok(observation?),
            },
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
