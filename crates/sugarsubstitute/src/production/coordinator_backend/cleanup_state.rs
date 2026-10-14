// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain cleanup causes and original grace/total wakes independently of transport health.
use std::{error::Error, io};
use sugarsubstitute_contracts::{execution::Moment, lifecycle::ShutdownBudget};

/// One failed cleanup never creates settlement or permits another attempt.
pub(super) struct CleanupState {
    first: Option<Box<dyn Error + Send + Sync>>,
    secondary: [Option<Box<dyn Error + Send + Sync>>; 2],
    omitted: u64,
    grace_observed: bool,
    expired: bool,
}
impl CleanupState {
    /// Construct inert cause slots under the existing backend actor storage owner.
    pub(super) const fn new() -> Self {
        Self {
            first: None,
            secondary: [None, None],
            omitted: 0,
            grace_observed: false,
            expired: false,
        }
    }
    /// Adopt each unexpected cause before another callback, preserving the first.
    pub(super) fn retain(&mut self, cause: Box<dyn Error + Send + Sync>) {
        if self.first.is_none() {
            self.first = Some(cause);
        } else if let Some(slot) = self.secondary.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    /// Borrow the exact first cause without extracting its native custody.
    pub(super) fn first(&self) -> Option<&(dyn Error + Send + Sync + 'static)> {
        self.first.as_deref()
    }
    /// Sample the original transaction without resetting it or abandoning any job.
    pub(super) fn observe(&mut self, now: Moment, original: ShutdownBudget) {
        self.grace_observed |= now >= original.graceful_until;
        self.expired |= now >= original.settle_until;
        if self.first.is_none() && self.expired {
            self.retain(Box::new(io::Error::new(
                io::ErrorKind::TimedOut,
                "Original backend cleanup expired with retained native custody",
            )));
        }
    }
    /// Expired cleanup is dormant; pending jobs and native watches still wake their owner.
    pub(super) fn next_deadline(&self, original: ShutdownBudget) -> Option<Moment> {
        if self.expired {
            None
        } else if self.grace_observed {
            Some(original.settle_until)
        } else {
            Some(original.graceful_until)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_grace_wakes_before_total_and_failure_never_renews_or_overwrites_cause()
    -> Result<(), Box<dyn Error>> {
        let original = ShutdownBudget::new(Moment::from_nanos(0))?;
        let mut state = CleanupState::new();
        assert_eq!(state.next_deadline(original), Some(original.graceful_until));
        state.observe(original.graceful_until, original);
        assert_eq!(state.next_deadline(original), Some(original.settle_until));
        state.observe(original.settle_until, original);
        assert_eq!(state.next_deadline(original), None);
        assert_eq!(
            state.first().and_then(|e| e.downcast_ref::<io::Error>()).map(io::Error::kind),
            Some(io::ErrorKind::TimedOut)
        );
        let pointer = state.first().map(std::ptr::from_ref);
        state.retain(Box::new(io::Error::from_raw_os_error(5)));
        state.observe(Moment::from_nanos(999_000_000_000), original);
        assert_eq!(state.first().map(std::ptr::from_ref), pointer);
        assert!(state.secondary[0].is_some());
        assert_eq!(state.next_deadline(original), None);
        Ok(())
    }
}
