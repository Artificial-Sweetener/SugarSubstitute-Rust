// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove retirement its owner never observed is announced after the scope can see it.

use super::*;
use crate::{
    runtime::{ExecutionClient, ExecutionOwner},
    runtime_test_support::{ClockPort, Gate, Held, ManualClock, TestResult, spec},
};
use std::{
    sync::{Condvar, OnceLock, Weak},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobClass, Moment, OwnedJob, Scope,
    ScopeSnapshot, ScopeSpec,
};

/// Act as the fastest consumer: drain on every wake, then record the scope it can observe.
/// A stale last snapshot is a lost wakeup, because coalescing ends at each drain.
#[derive(Default)]
struct ScopeWake {
    scope: OnceLock<(Weak<Core>, ScopeId)>,
    observed: Mutex<Vec<ScopeSnapshot>>,
    changed: Condvar,
}
impl ScopeWake {
    fn count(&self) -> usize {
        lock(&self.observed).len()
    }
    /// Bound a stalled observer; a missing wake fails instead of hanging.
    fn wait_until(
        &self,
        satisfied: impl Fn(&[ScopeSnapshot]) -> bool,
        failure: &str,
    ) -> TestResult<Vec<ScopeSnapshot>> {
        let (observed, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.observed), Duration::from_secs(10), |observed| {
                !satisfied(observed)
            })
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && !satisfied(&observed) {
            return Err(format!("{failure}: {:?}", observed.last()).into());
        }
        Ok(observed.clone())
    }
}
impl CompletionWake for ScopeWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        if let Some((core, id)) = self.scope.get()
            && let Some(core) = core.upgrade()
        {
            core.mailbox.drain(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            let snapshot = core.scope_snapshot(*id);
            lock(&self.observed).push(snapshot);
        }
        self.changed.notify_all();
        Ok(())
    }
}

/// Retired records are announced for every scope; byte release only settles a cancelled one.
fn drained(snapshot: &ScopeSnapshot) -> bool {
    snapshot.queued == 0
        && snapshot.running == 0
        && snapshot.terminal_unobserved == 0
        && (!snapshot.cancellation_requested || snapshot.reserved_bytes == 0)
}

/// Submit one job that holds its worker between the two gates; the caller abandons it.
type Submit<J> = fn(&ExecutionClient, &Scope, u64, Arc<Gate>, Arc<Gate>) -> TestResult<J>;

fn held_blocking(
    client: &ExecutionClient,
    scope: &Scope,
    bytes: u64,
    entered: Arc<Gate>,
    release: Arc<Gate>,
) -> TestResult<OwnedJob<(), std::io::Error>> {
    Ok(client.submit_blocking(
        scope,
        spec(1, JobClass::InfrastructureOrdinary, bytes)?,
        move |_| {
            entered.open();
            release.wait().map_err(|cause| std::io::Error::other(cause.to_string()))
        },
    )?)
}

fn held_cpu(
    client: &ExecutionClient,
    scope: &Scope,
    bytes: u64,
    entered: Arc<Gate>,
    release: Arc<Gate>,
) -> TestResult<OwnedJob<(), String>> {
    Ok(client.submit_cpu(scope, spec(1, JobClass::CpuInteractive, bytes)?, move |_| Held {
        entered,
        release,
    })?)
}

/// Abandon a held job, release it and require a wake that sees its record gone.
fn abandoned_result_retirement_is_announced<J>(
    submit: Submit<J>,
    cancel_scope: bool,
    bytes: u64,
) -> TestResult {
    let wake = Arc::new(ScopeWake::default());
    let mut owner = ExecutionOwner::with_cpu_workers(
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        wake.clone(),
        2,
    )?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    assert!(wake.scope.set((Arc::downgrade(&client.core), scope.id())).is_ok());
    let (entered, release) = (Arc::new(Gate::default()), Arc::new(Gate::default()));
    let job = submit(&client, &scope, bytes, entered.clone(), release.clone())?;
    entered.wait()?;
    drop(job);
    if cancel_scope {
        scope.cancel();
    }
    release.open();
    wake.wait_until(
        |observed| observed.last().is_some_and(drained),
        "Retirement of the abandoned result produced no wake after its record left the scope",
    )?;
    drop(scope);
    owner.request_shutdown(Moment::from_nanos(0));
    loop {
        let observed = wake.count();
        if owner.poll_shutdown()?.settled {
            break;
        }
        wake.wait_until(|recorded| recorded.len() > observed, "Shutdown produced no wake")?;
    }
    owner.finish_shutdown()?;
    Ok(())
}

#[test]
fn abandoned_blocking_retirement_wakes_a_live_scope_observer() -> TestResult {
    abandoned_result_retirement_is_announced(held_blocking, false, 4096)
}

#[test]
fn abandoned_blocking_retirement_wakes_a_cancelled_scope_observer() -> TestResult {
    abandoned_result_retirement_is_announced(held_blocking, true, 0)
}

#[test]
fn abandoned_cpu_retirement_wakes_a_live_scope_observer() -> TestResult {
    abandoned_result_retirement_is_announced(held_cpu, false, 4096)
}

#[test]
fn abandoned_cpu_retirement_wakes_a_cancelled_scope_observer() -> TestResult {
    abandoned_result_retirement_is_announced(held_cpu, true, 4096)
}
