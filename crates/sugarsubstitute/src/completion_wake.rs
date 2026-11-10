// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Let tests block on the execution owner's delivered events instead of elapsed time.
//!
//! Execution owners publish one completion wake for every terminal job publication and one
//! after the last worker returns from shutdown. A test that observes [`Wake::revision`] before it
//! inspects owned state and then calls [`Wake::wait_after`] therefore cannot miss an event and
//! needs no wall-clock bound: real work that is slow under machine load only makes the test
//! slower, never failed. A lost wake is an execution-owner defect, proven by that crate's own
//! tests, and would show as a test that libtest reports as running long.

use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobCompletion, Moment, OwnedJob,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};

/// Completions delivered to the observer per drain, matching the host's per-poll bound.
const DRAIN_BOUND: usize = 32;

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

    /// Deliver completions until `job` has its typed terminal result, then take it.
    ///
    /// # Errors
    /// Returns the execution owner's delivery failure.
    pub(crate) fn completion<T, E>(
        &self,
        client: &ExecutionClient,
        job: &mut OwnedJob<T, E>,
    ) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
        loop {
            let observed = self.revision();
            self.drain(client)?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            self.wait_after(observed);
        }
    }

    /// Shut `owner` down at `cutoff`, delivering completions until every worker has returned.
    ///
    /// # Errors
    /// Returns the execution owner's delivery, observation or teardown failure.
    pub(crate) fn finish_shutdown(
        &self,
        owner: &mut ExecutionOwner,
        client: &ExecutionClient,
        cutoff: Moment,
    ) -> Result<(), Box<dyn Error>> {
        owner.request_shutdown(cutoff);
        loop {
            let observed = self.revision();
            self.drain(client)?;
            if owner.poll_shutdown()?.settled {
                break;
            }
            self.wait_after(observed);
        }
        owner.finish_shutdown()?;
        Ok(())
    }

    fn drain(&self, client: &ExecutionClient) -> Result<(), ExecutionError> {
        let bound = NonZeroUsize::MIN.saturating_add(DRAIN_BOUND - 1);
        client.drain_completions(bound).map(drop)
    }
}
