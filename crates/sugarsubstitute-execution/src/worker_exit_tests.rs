// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove worker return is announced once, after it is observable, and never early.

use super::*;
use crate::{
    runtime::ExecutionOwner,
    runtime_test_support::{ClockPort, Gate, ManualClock, TestResult, Wake, spec},
};
use std::{
    sync::{Condvar, OnceLock, Weak},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, CpuStep, CpuTask, ExecutionError, ExecutionId, ExecutionPort, JobClass,
    JobContext, JobOutcome, Moment, ScopeId, ScopeSpec,
};

/// Record the census each wake can observe, proving the wake follows the published status.
#[derive(Default)]
struct StatusWake {
    core: OnceLock<Weak<Core>>,
    observed: Mutex<Vec<Option<WorkerStatus>>>,
    changed: Condvar,
}
impl StatusWake {
    fn observed(&self) -> Vec<Option<WorkerStatus>> {
        lock(&self.observed).clone()
    }
    /// Bound a stalled observer; a missing settlement wake fails instead of hanging.
    fn wait_after(&self, observed: usize) -> TestResult {
        let (recorded, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.observed), Duration::from_secs(10), |recorded| {
                recorded.len() == observed
            })
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && recorded.len() == observed {
            return Err("Worker settlement produced no wake".into());
        }
        Ok(())
    }
}
impl CompletionWake for StatusWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let status = self.core.get().and_then(Weak::upgrade).map(|core| core.workers.status());
        lock(&self.observed).push(status);
        self.changed.notify_all();
        Ok(())
    }
}

const FINISHED: WorkerStatus =
    WorkerStatus { native_finished: true, async_finished: true, cpu_finished: true };

/// Keep one CPU lane inside its loop until the test opens the barrier.
struct Held {
    entered: Arc<Gate>,
    release: Arc<Gate>,
}
impl CpuTask<(), String> for Held {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, String> {
        self.entered.open();
        self.release.wait().map_err(|cause| cause.to_string())?;
        Ok(CpuStep::Complete(()))
    }
}

#[test]
fn only_the_last_return_wakes_and_its_status_is_already_visible() -> TestResult {
    let wake = Arc::new(StatusWake::default());
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        wake.clone(),
        crate::runtime::DEFAULT_MEMORY_BYTES,
    );
    assert!(wake.core.set(Arc::downgrade(&core)).is_ok());
    let native = WorkerExit::enlist(&core, WorkerKind::Native);
    let cpu =
        [WorkerExit::enlist(&core, WorkerKind::Cpu), WorkerExit::enlist(&core, WorkerKind::Cpu)];
    let supervisor = WorkerExit::enlist(&core, WorkerKind::Async);
    drop(native);
    let [first, second] = cpu;
    drop(first);
    assert_eq!(
        core.workers.status(),
        WorkerStatus { native_finished: true, async_finished: false, cpu_finished: false }
    );
    drop(supervisor);
    assert!(wake.observed().is_empty(), "a CPU lane is still inside its loop");
    drop(second);
    assert_eq!(wake.observed(), vec![Some(FINISHED)]);
    Ok(())
}

#[test]
fn shutdown_wake_waits_for_a_worker_held_in_its_loop() -> TestResult {
    let wake = Arc::new(StatusWake::default());
    let mut owner = ExecutionOwner::with_cpu_workers(
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        wake.clone(),
        2,
    )?;
    let client = owner.client();
    assert!(wake.core.set(Arc::downgrade(&client.core)).is_ok());
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let (entered, release) = (Arc::new(Gate::default()), Arc::new(Gate::default()));
    let (task_entered, task_release) = (entered.clone(), release.clone());
    let mut job = client.submit_cpu(&scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
        Held { entered: task_entered, release: task_release }
    })?;
    entered.wait()?;
    owner.request_shutdown(Moment::from_nanos(0));
    let held = owner.poll_shutdown()?;
    assert!(!held.settled);
    assert!(!held.workers.cpu_finished);
    assert!(!wake.observed().contains(&Some(FINISHED)));
    release.open();
    let mut settled = false;
    while !settled {
        let observed = wake.observed().len();
        client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            assert!(matches!(completion.outcome, JobOutcome::Cancelled));
        }
        settled = owner.poll_shutdown()?.settled;
        if !settled {
            wake.wait_after(observed)?;
        }
    }
    owner.finish_shutdown()?;
    let finished: Vec<_> =
        wake.observed().into_iter().filter(|status| *status == Some(FINISHED)).collect();
    assert_eq!(finished.len(), 1);
    assert_eq!(wake.observed().last(), Some(&Some(FINISHED)));
    Ok(())
}

#[test]
fn repeated_shutdown_requests_deliver_exactly_one_worker_wake() -> TestResult {
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::with_cpu_workers(
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        wake.clone(),
        2,
    )?;
    owner.request_shutdown(Moment::from_nanos(0));
    owner.request_shutdown(Moment::from_nanos(0));
    loop {
        let observed = wake.count();
        if owner.poll_shutdown()?.settled {
            break;
        }
        wake.wait_after(observed)?;
    }
    owner.finish_shutdown()?;
    assert_eq!(wake.count(), 1);
    Ok(())
}
