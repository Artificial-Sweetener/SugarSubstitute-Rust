// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Let tests block on the execution owner's delivered events instead of elapsed time.
//!
//! Execution owners publish one completion wake for every terminal job publication and one
//! after the last worker returns from shutdown. A test that observes [`Wake::revision`] before it
//! inspects owned state and then calls [`Wake::wait_after`] therefore cannot miss an event and
//! needs no wall-clock bound: real work that is slow under machine load only makes the test
//! slower, never failed.
//!
//! Every wait here is also watched against a lost event, which is a defect and not a slow
//! machine. Waits share the process-wide [`ProgressGuard`]: progress is any wake or released gate
//! anywhere in the test process, and only when nothing at all has progressed for the guard's stall
//! does a waiter fail, naming the event it never received. Elapsed time never decides a healthy
//! test: parallel tests keep the progress mark fresh for as long as any of them works.

use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobCompletion, Moment, OwnedJob,
};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, qualification_time::ProgressGuard,
};

/// Completions delivered to the observer per drain, matching the host's per-poll bound.
const DRAIN_BOUND: usize = 32;

/// Block on `changed` while `waiting` holds, failing with a diagnostic once the process has made
/// no progress for the stall. `what` names the awaited event only when it is reported.
pub(crate) fn wait_while_watched<'a, T>(
    changed: &Condvar,
    guard: MutexGuard<'a, T>,
    what: impl Fn() -> String,
    waiting: impl FnMut(&mut T) -> bool,
) -> MutexGuard<'a, T> {
    let watched = ProgressGuard::process().wait_while(changed, guard, waiting);
    assert!(
        watched.stalled.is_none(),
        "stalled waiting for {}; in this test process {}",
        what(),
        watched.stalled.map(|stall| stall.to_string()).unwrap_or_default(),
    );
    watched.guard
}

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
        ProgressGuard::process().note();
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
    /// Callers wait only while real work that will publish a wake is outstanding.
    pub(crate) fn wait_after(&self, observed: u64) {
        drop(wait_while_watched(
            &self.changed,
            self.locked(),
            || format!("a completion wake after revision {observed}"),
            |revision| *revision == observed,
        ));
    }

    /// Re-examine `ready` after each wake until it yields a value.
    ///
    /// `ready` runs after the revision is snapshotted, so an event that lands while it inspects
    /// state is never missed.
    pub(crate) fn until<T>(&self, mut ready: impl FnMut() -> Option<T>) -> T {
        loop {
            let observed = self.revision();
            if let Some(value) = ready() {
                return value;
            }
            self.wait_after(observed);
        }
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
        self.until(|| match self.drain(client) {
            Err(cause) => Some(Err(cause.into())),
            Ok(()) => job.try_take_completion().map(Ok),
        })
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
        self.until(|| {
            let settled = self.drain(client).map_err(Box::<dyn Error>::from).and_then(|()| {
                owner.poll_shutdown().map(|progress| progress.settled).map_err(Into::into)
            });
            match settled {
                Ok(false) => None,
                other => Some(other),
            }
        })?;
        owner.finish_shutdown()?;
        Ok(())
    }

    fn drain(&self, client: &ExecutionClient) -> Result<(), ExecutionError> {
        let bound = NonZeroUsize::MIN.saturating_add(DRAIN_BOUND - 1);
        client.drain_completions(bound).map(drop)
    }
}

/// Entry and release of one held worker, observed without a clock.
#[derive(Default)]
struct GateState {
    entered: bool,
    released: bool,
}

/// Hold a worker at a finite external boundary until the test releases it.
///
/// Wrap it in a drop guard that calls [`Gate::release`] so a failing test can never leave the
/// worker blocked while its executor waits to join it.
#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}

impl Gate {
    fn locked(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Worker side: publish entry, then hold until released.
    pub(crate) fn hold(&self) {
        let mut state = self.locked();
        state.entered = true;
        self.changed.notify_all();
        drop(wait_while_watched(
            &self.changed,
            state,
            || "the test to release a held worker".to_owned(),
            |state| !state.released,
        ));
    }

    /// Test side: block until a worker has entered the gate.
    pub(crate) fn wait_entered(&self) {
        drop(wait_while_watched(
            &self.changed,
            self.locked(),
            || "a worker to enter its gate".to_owned(),
            |state| !state.entered,
        ));
    }

    /// Let the held worker return; repeated release is harmless.
    pub(crate) fn release(&self) {
        self.locked().released = true;
        ProgressGuard::process().note();
        self.changed.notify_all();
    }
}

/// Releases its gate when dropped, so a failing assertion can never leave a held worker blocking
/// the executor join that follows.
pub(crate) struct ReleaseOnDrop(Arc<Gate>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

impl Gate {
    /// Guard that releases this gate on drop, including when the test unwinds.
    pub(crate) fn guard(self: &Arc<Self>) -> ReleaseOnDrop {
        ReleaseOnDrop(Arc::clone(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paused_clock::PausedClock;
    use std::sync::Arc;
    use sugarsubstitute_contracts::execution::{
        JobClass, JobId, JobOutcome, JobSpec, Operation, ScopeId, ScopeSpec,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    /// A held worker is observed entering, then returns only after its guard or release frees it.
    #[test]
    fn a_gate_holds_a_real_worker_until_released() -> TestResult {
        let clock = PausedClock::new();
        let wake = Arc::new(Wake::default());
        let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let gate = Arc::new(Gate::default());
        let held = gate.clone();
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: 1024,
            replacement: None,
        };
        let mut job = client.submit_blocking(&scope, spec, move |_| {
            held.hold();
            Ok::<_, std::io::Error>(7)
        })?;
        gate.wait_entered();
        assert!(job.try_take_completion().is_none());
        // Dropping the guard releases the worker; releasing again is harmless.
        drop(gate.guard());
        gate.release();
        assert!(matches!(wake.completion(&client, &mut job)?.outcome, JobOutcome::Success(7)));
        drop((job, scope));
        wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(1))?;
        Ok(())
    }
}
