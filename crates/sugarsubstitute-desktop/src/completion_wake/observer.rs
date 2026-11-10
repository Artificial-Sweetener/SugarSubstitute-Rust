// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Let tests block on the execution owner's delivered events instead of elapsed time.
//!
//! Execution owners publish one completion wake for every terminal job publication and one after
//! the last worker returns from shutdown. A test that snapshots [`Wake::revision`] before it
//! inspects owned state and then calls [`Wake::wait_after`] therefore cannot miss an event and
//! needs no wall-clock bound: real work that is slow under machine load only makes the test
//! slower, never failed. A lost wake is an execution-owner defect, proven by that crate's own
//! tests, and would show as a test that libtest reports as running long.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};

/// Count completion wakes so an observer can sleep until the next real execution event.
#[derive(Default)]
pub(crate) struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}

impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.locked();
        *revision = revision.saturating_add(1);
        drop(revision);
        self.changed.notify_all();
        Ok(())
    }
}

impl Wake {
    fn locked(&self) -> MutexGuard<'_, u64> {
        self.revision.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Snapshot the wake count before inspecting state, so an event after the snapshot is kept.
    pub(crate) fn revision(&self) -> u64 {
        *self.locked()
    }

    /// Block until a wake newer than `observed` has been delivered.
    ///
    /// Callers wait only while real work that will publish a wake is outstanding. The wait has no
    /// wall-clock bound, so scheduling or storage delay never changes a test's outcome.
    pub(crate) fn wait_after(&self, observed: u64) {
        drop(
            self.changed
                .wait_while(self.locked(), |revision| *revision == observed)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    /// A wake delivered between the snapshot and the wait is kept, so no event can be missed.
    #[test]
    fn a_wake_delivered_after_the_snapshot_ends_the_wait_without_blocking()
    -> Result<(), ExecutionError> {
        let wake = Wake::default();
        let observed = wake.revision();
        wake.wake()?;
        wake.wait_after(observed);
        assert_eq!(wake.revision(), observed + 1);
        Ok(())
    }

    /// Each delivered wake advances the revision exactly once, whoever observed it.
    #[test]
    fn every_delivered_wake_advances_the_revision_by_exactly_one() -> Result<(), ExecutionError> {
        let wake = Wake::default();
        let first = wake.revision();
        for delivered in 1..=3 {
            wake.wake()?;
            assert_eq!(wake.revision(), first + delivered);
        }
        Ok(())
    }

    /// An observer that panicked while holding the revision must not turn later wakes into losses.
    #[test]
    fn a_poisoned_revision_still_delivers_and_observes_wakes() -> Result<(), ExecutionError> {
        let wake = Wake::default();
        let poisoned: Result<(), _> = catch_unwind(AssertUnwindSafe(|| {
            let _held = wake.locked();
            resume_unwind(Box::new("revision holder failed"));
        }));
        assert!(poisoned.is_err());
        let observed = wake.revision();
        wake.wake()?;
        wake.wait_after(observed);
        assert_eq!(wake.revision(), observed + 1);
        Ok(())
    }
}
