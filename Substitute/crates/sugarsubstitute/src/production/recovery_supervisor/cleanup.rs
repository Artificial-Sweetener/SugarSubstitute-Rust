// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Freeze capture before containment and preserve each original cutoff across later turns.
use sugarsubstitute_contracts::{
    crash::CaptureBudget,
    execution::Moment,
    lifecycle::{LifecycleRejection, ShutdownBudget},
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Dormant,
    Capture(CaptureBudget),
    Containment(ShutdownBudget),
}
/// A stopped transport never shortens fatal capture by starting execution shutdown immediately.
/// Only the authenticated Root's explicit Stop may finish its capture policy earlier.
pub(super) struct Cleanup {
    phase: Phase,
}
impl Cleanup {
    pub(super) const fn new() -> Self {
        Self { phase: Phase::Dormant }
    }
    /// Latch the first unexpected Root/client loss without renewing an existing transaction.
    pub(super) fn capture(
        &mut self,
        now: Moment,
    ) -> Result<(), sugarsubstitute_contracts::execution::TimeError> {
        if self.phase == Phase::Dormant {
            self.phase = Phase::Capture(CaptureBudget::new(now)?);
        }
        Ok(())
    }
    /// Apply the genuine Root's original cleanup cutoff, at most the existing15 seconds.
    /// Later Stop frames may shorten the same transaction and can never extend it.
    pub(super) fn root_stop(
        &mut self,
        now: Moment,
        until: Moment,
    ) -> Result<(), LifecycleRejection> {
        let mut budget = match self.phase {
            Phase::Containment(budget) => budget,
            _ => ShutdownBudget::new(now)?,
        };
        budget.settle_until = budget.settle_until.min(until);
        budget.graceful_until = budget.graceful_until.min(budget.settle_until);
        self.phase = Phase::Containment(budget);
        Ok(())
    }
    /// Start containment from the original capture cutoff, including when a busy turn arrives late.
    pub(super) fn advance(
        &mut self,
        now: Moment,
    ) -> Result<Option<ShutdownBudget>, LifecycleRejection> {
        if let Phase::Capture(capture) = self.phase
            && now >= capture.until()
        {
            self.phase = Phase::Containment(ShutdownBudget::new(capture.until())?);
        }
        Ok(self.containment())
    }
    pub(super) const fn containment(&self) -> Option<ShutdownBudget> {
        match self.phase {
            Phase::Containment(budget) => Some(budget),
            _ => None,
        }
    }
    /// Original finite event-write cutoff; it is not an idle/read timer or startup delegation.
    pub(super) const fn until(&self) -> Option<Moment> {
        match self.phase {
            Phase::Dormant => None,
            Phase::Capture(budget) => Some(budget.until()),
            Phase::Containment(budget) => Some(budget.settle_until),
        }
    }
    pub(super) const fn requested(&self) -> bool {
        !matches!(self.phase, Phase::Dormant)
    }
}
#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod tests;
