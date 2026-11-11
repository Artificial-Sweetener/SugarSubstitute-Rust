// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run one storage job on manual time while the test thread holds or releases short locks.
//!
//! The job's clock moves only when the driver advances it, so a lock wait times out exactly when
//! the test reaches its deadline and never because the machine is slow. The driver runs on the
//! test thread with the job's cancel control; a [`Handoff`] orders their steps without sleeps.
use super::*;
use std::sync::Condvar;
use sugarsubstitute_execution::clock_fixture::ManualClock;

/// Ordered rendezvous between the storage job and the driver on the test thread.
#[derive(Default)]
pub(crate) struct Handoff {
    step: Mutex<u32>,
    changed: Condvar,
}
impl Handoff {
    /// Record that `step` has been reached.
    pub(crate) fn reach(&self, step: u32) {
        let mut current = crate::registry::lock(&self.step);
        *current = (*current).max(step);
        self.changed.notify_all();
    }
    /// Block until `step` has been reached; the other side always reaches it or the job ends.
    pub(crate) fn wait(&self, step: u32) {
        drop(
            self.changed
                .wait_while(crate::registry::lock(&self.step), |current| *current < step)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
}

/// Test-thread controls for one driven storage job.
pub(crate) struct Driver<'a> {
    /// The job's manual clock; nothing else advances it.
    pub(crate) clock: &'a ManualClock,
    /// Shared ordering with the job body.
    pub(crate) handoff: &'a Handoff,
    cancel: &'a dyn Fn(),
}
impl Driver<'_> {
    /// Request cancellation of the running storage job.
    pub(crate) fn cancel(&self) {
        (self.cancel)();
    }
}

/// A job deadline far beyond any lock wait; manual time reaches it only if a driver goes there.
const JOB_DEADLINE: Moment = Moment::from_nanos(1_000_000_000_000);

/// Run `body` as one blocking storage job on a fresh manual clock while `driver` runs here.
/// The job deadline lies far beyond the lock wait bound, so only that bound expires first.
pub(crate) fn run_driven(
    reservation_bytes: u64,
    body: impl FnOnce(&JobContext, &Handoff) -> TestResult + Send + 'static,
    driver: impl FnOnce(&Driver<'_>) -> TestResult,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = ManualClock::new();
    let handoff = Arc::new(Handoff::default());
    let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let observed = Arc::new(Mutex::new(None));
    let worker_observed = observed.clone();
    let worker_handoff = handoff.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(JOB_DEADLINE),
            reservation_bytes,
            replacement: None,
        },
        move |context| {
            let result = body(&context, &worker_handoff);
            worker_handoff.reach(u32::MAX);
            *crate::registry::lock(&worker_observed) = Some(result);
            Ok::<_, io::Error>(())
        },
    )?;
    let control = &job;
    let driven = driver(&Driver { clock: &clock, handoff: &handoff, cancel: &|| control.cancel() });
    let completion = loop {
        let revision = *crate::registry::lock(&wake.0);
        client.drain_completions(
            std::num::NonZeroUsize::new(32).ok_or("Invalid completion bound")?,
        )?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        drop(
            wake.1
                .wait_while(crate::registry::lock(&wake.0), |current| *current == revision)
                .map_err(|_| "Poisoned storage completion wake")?,
        );
    };
    drop(job);
    drop(executor);
    crate::registry::lock(&observed).take().ok_or("Storage assertions did not complete")??;
    driven?;
    match completion.outcome {
        JobOutcome::Success(()) | JobOutcome::Cancelled => Ok(()),
        JobOutcome::Failure(cause) => Err(cause.into()),
        other => Err(format!("Unexpected storage outcome: {:?}", other.kind()).into()),
    }
}
