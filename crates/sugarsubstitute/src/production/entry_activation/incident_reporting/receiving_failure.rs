// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a bounded receiving cleanup cause independently of the original transition failure.
use super::*;
/// One actual later error remains owned with its failed receiving transition.
/// Further errors are counted; native custody remains with the existing operation owners.
pub(super) struct SecondaryFailure {
    pub(super) transition: ReportTransition,
    pub(super) cause: Box<dyn Error>,
}
impl IncidentReporting {
    /// Adopt before formatting, preserving the first cause and the existing first-error return.
    /// Emit at most one primary, one retained secondary and one omission diagnostic per owner.
    /// Repeated failures grant no retry, settlement, fresh admission or deadline authority.
    pub(super) fn retain_transition_failure(
        &mut self,
        transition: ReportTransition,
        cause: Box<dyn Error>,
    ) -> Result<()> {
        if self.first.is_none() {
            let retained = self.first.insert(cause);
            tracing::error!(
                operation = "reporter_transition_failed",
                transition = ?transition,
                failure = ?crate::production::entry_runner::FailureDisplay(retained.as_ref()),
                "Actual Reporter receiving transition failed with original cause retained"
            );
            return Err("Actual report receiving failed with retained original custody".into());
        }
        if self.secondary_receiving_failure.is_none() {
            let retained =
                self.secondary_receiving_failure.insert(SecondaryFailure { transition, cause });
            tracing::error!(
                operation = "reporter_secondary_transition_failed",
                transition = ?retained.transition,
                failure = ?crate::production::entry_runner::FailureDisplay(retained.cause.as_ref()),
                "Additional Reporter receiving cause retained independently of the first failure"
            );
        } else {
            self.omitted_receiving_failures = self.omitted_receiving_failures.saturating_add(1);
            if self.omitted_receiving_failures == 1 {
                tracing::error!(
                    operation = "reporter_transition_failures_omitted",
                    transition = ?transition,
                    omitted = self.omitted_receiving_failures,
                    "Further Reporter receiving failures are counted; original causes remain retained"
                );
            }
        }
        Ok(())
    }
}
