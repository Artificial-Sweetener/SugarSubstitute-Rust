// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministic checkpoint debounce, independent of runtime timers.
use std::time::Duration;
use sugarsubstitute_contracts::execution::{Moment, TimeError};

/// Desktop session autosave timings, in monotonic execution-clock units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutosavePolicy {
    /// Quiet time after the latest observation.
    pub quiet: Duration,
    /// Maximum dirty time while observations keep arriving.
    pub longest: Duration,
}
impl Default for AutosavePolicy {
    fn default() -> Self {
        Self { quiet: Duration::from_millis(250), longest: Duration::from_secs(2) }
    }
}
#[derive(Default)]
pub(super) struct Schedule {
    first: Option<Moment>,
    due: Option<Moment>,
}
impl Schedule {
    pub(super) fn changed(&mut self, now: Moment) -> Result<(), TimeError> {
        let policy = AutosavePolicy::default();
        let first = self.first.unwrap_or(now);
        let due = now.checked_add(policy.quiet)?.min(first.checked_add(policy.longest)?);
        self.first = Some(first);
        self.due = Some(due);
        Ok(())
    }
    pub(super) fn force(&mut self, now: Moment) {
        self.first.get_or_insert(now);
        self.due = Some(now);
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.due
    }
    pub(super) fn dirty(&self) -> bool {
        self.due.is_some()
    }
    pub(super) fn take(&mut self) {
        self.first = None;
        self.due = None;
    }
}
