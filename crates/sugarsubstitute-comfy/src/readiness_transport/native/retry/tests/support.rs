// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe retry assertions inside genuine finite execution and cancellation lifetimes.
use std::{
    io,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobContext, JobId, JobOutcome,
    JobSpec, Moment, Operation, Scope, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;

/// Preserve assertion and observation failures across the actual worker boundary.
pub(super) type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
/// Select the genuine job condition observed by the retry classifier.
#[derive(Clone, Copy)]
pub(super) enum State {
    /// Keep the original admitted deadline and uncancelled scope.
    Live,
    /// Cancel the actual scope after its worker has entered.
    Cancelled,
    /// Advance the injected clock to the original finite deadline inside the worker.
    Expired,
}
/// Supply a deterministic monotonic boundary without creating another timer or executor.
#[derive(Default)]
struct ManualClock(AtomicU64);
impl Clock for ManualClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Wake the test only through the production executor's completion notifications.
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.0) += 1;
        self.1.notify_all();
        Ok(())
    }
}
/// Retain test cleanup access if an assertion poisoned its independently observed state.
fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Run assertions before actual worker termination, then observe the original terminal outcome.
pub(super) fn observe(
    state: State,
    body: impl FnOnce(&JobContext) -> TestResult + Send + 'static,
) -> TestResult {
    let clock = Arc::new(ManualClock::default());
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let scope =
        Arc::new(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
    let worker_scope: Arc<Scope> = scope.clone();
    let observed = Arc::new(Mutex::new(None));
    let worker_observed = observed.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(2_000_000_000)),
            reservation_bytes: 4096,
            replacement: None,
        },
        move |context| {
            match state {
                State::Live => {}
                State::Cancelled => worker_scope.cancel(),
                State::Expired => clock.0.store(2_000_000_000, Ordering::Release),
            }
            *lock(&worker_observed) = Some(body(&context));
            Ok::<(), io::Error>(())
        },
    )?;
    let until = Instant::now() + Duration::from_secs(5);
    let completion = loop {
        let revision = *lock(&wake.0);
        client.drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid drain")?)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let remaining =
            until.checked_duration_since(Instant::now()).ok_or("Retry fixture did not settle")?;
        let (guard, _) = wake
            .1
            .wait_timeout_while(lock(&wake.0), remaining, |current| *current == revision)
            .map_err(|_| "Retry fixture wake poisoned")?;
        drop(guard);
    };
    lock(&observed).take().ok_or("Retry assertions were not executed")??;
    assert!(matches!(
        (state, completion.outcome),
        (State::Live, JobOutcome::Success(()))
            | (State::Cancelled, JobOutcome::Cancelled)
            | (State::Expired, JobOutcome::TimedOut)
    ));
    drop(job);
    drop(scope);
    drop(client);
    drop(executor);
    Ok(())
}
