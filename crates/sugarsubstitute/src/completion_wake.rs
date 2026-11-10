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
//! machine. A blocked waiter re-examines process-wide progress every [`PATROL`]; progress is any
//! wake or released gate anywhere in the test process. Only when nothing at all has progressed for
//! [`STALL`] does the waiter fail, naming the event it never received, so a lost
//! wake ends as a diagnosed failure instead of a hang. Elapsed time never decides a healthy test:
//! parallel tests keep the process-wide progress mark fresh for as long as any of them works.

use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobCompletion, Moment, OwnedJob,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};

/// Completions delivered to the observer per drain, matching the host's per-poll bound.
const DRAIN_BOUND: usize = 32;
/// How often a blocked waiter samples process-wide progress. It decides nothing by itself.
const PATROL: Duration = Duration::from_secs(15);
/// Process-wide silence that proves an event was lost rather than slow. It is far above the
/// longest single job seen on a starved machine, and every unrelated wake resets it.
const STALL: Duration = Duration::from_secs(10 * 60);

/// Remember when anything in the test process last made progress.
struct Progress {
    origin: OnceLock<Instant>,
    last: Mutex<Option<Instant>>,
}

impl Progress {
    const fn new() -> Self {
        Self { origin: OnceLock::new(), last: Mutex::new(None) }
    }

    fn note(&self) {
        let now = Instant::now();
        self.origin.get_or_init(|| now);
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Some(now);
    }

    /// Start counting silence from the first wait, so a process that never progresses still
    /// reaches the stall.
    fn begin(&self) {
        self.origin.get_or_init(Instant::now);
    }

    /// Time since the latest progress, or since the first wait when none was noted.
    fn silence(&self) -> Duration {
        let last = *self.last.lock().unwrap_or_else(PoisonError::into_inner);
        let since = last.or_else(|| self.origin.get().copied()).unwrap_or_else(Instant::now);
        since.elapsed()
    }
}

/// Progress shared by every wake and gate of the test binary.
static PROGRESS: Progress = Progress::new();

/// Sampling cadence and the process-wide silence that turns a wait into a failure.
#[derive(Clone, Copy)]
struct Watch {
    patrol: Duration,
    stall: Duration,
}

/// The cadence every ordinary test uses.
const WATCH: Watch = Watch { patrol: PATROL, stall: STALL };

/// Block on `changed` while `waiting` holds, failing with a diagnostic once the process has made
/// no progress for the stall. `what` names the awaited event only when it is reported.
pub(crate) fn wait_while_watched<'a, T>(
    changed: &Condvar,
    guard: MutexGuard<'a, T>,
    what: impl Fn() -> String,
    waiting: impl FnMut(&mut T) -> bool,
) -> MutexGuard<'a, T> {
    watched(&PROGRESS, WATCH, changed, guard, &what, waiting)
}

fn watched<'a, T>(
    progress: &Progress,
    watch: Watch,
    changed: &Condvar,
    mut guard: MutexGuard<'a, T>,
    what: &dyn Fn() -> String,
    mut waiting: impl FnMut(&mut T) -> bool,
) -> MutexGuard<'a, T> {
    progress.begin();
    loop {
        let (next, result) = changed
            .wait_timeout_while(guard, watch.patrol, &mut waiting)
            .unwrap_or_else(PoisonError::into_inner);
        guard = next;
        if !result.timed_out() {
            progress.note();
            return guard;
        }
        let silence = progress.silence();
        assert!(
            silence < watch.stall,
            "stalled waiting for {}; nothing in this test process made progress for {silence:?}",
            what(),
        );
    }
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
        PROGRESS.note();
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
        PROGRESS.note();
        self.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paused_clock::PausedClock;
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::Arc,
    };
    use sugarsubstitute_contracts::execution::{
        JobClass, JobId, JobOutcome, JobSpec, Operation, ScopeId, ScopeSpec,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    /// A lost event ends as a failure that names the missing event.
    #[test]
    fn a_wait_with_no_progress_anywhere_fails_naming_what_never_arrived() {
        let progress = Progress::new();
        let watch = Watch { patrol: Duration::from_millis(5), stall: Duration::from_millis(40) };
        let changed = Condvar::new();
        let state = Mutex::new(false);
        let failure = catch_unwind(AssertUnwindSafe(|| {
            drop(watched(
                &progress,
                watch,
                &changed,
                state.lock().unwrap_or_else(PoisonError::into_inner),
                &|| "the lost fixture event".to_owned(),
                |released| !*released,
            ));
        }));
        let message = failure
            .err()
            .and_then(|payload| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        assert!(message.contains("stalled waiting for the lost fixture event"), "{message}");
    }

    /// A held worker is observed entering, then returns only after its release.
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
        gate.release();
        gate.release();
        assert!(matches!(wake.completion(&client, &mut job)?.outcome, JobOutcome::Success(7)));
        drop((job, scope));
        wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(1))?;
        Ok(())
    }
}
