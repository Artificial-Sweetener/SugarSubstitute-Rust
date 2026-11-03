// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove each ordinary path releases its reservation once its terminal outcome is visible.

use super::*;
use crate::{admission::RequestKind, runtime_test_support::*};
use sugarsubstitute_contracts::execution::*;

const BYTES: u64 = 64 * 1024;
/// Repetitions per path; one late worker-side context is enough to fail an iteration.
const ROUNDS: u64 = 32;

/// Finish in one step.
struct Immediate;
impl CpuTask<(), ()> for Immediate {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, ()> {
        Ok(CpuStep::Complete(()))
    }
}

/// Yield once, so the lane keeps the work across a round before it finishes.
struct YieldOnce(bool);
impl CpuTask<(), ()> for YieldOnce {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, ()> {
        if std::mem::replace(&mut self.0, true) {
            Ok(CpuStep::Complete(()))
        } else {
            Ok(CpuStep::Yield)
        }
    }
}

/// Take the visible completion and require that only it still holds the reservation.
fn released_at_terminal<T, E>(rig: &Rig, job: &mut OwnedJob<T, E>) -> TestResult<JobOutcome<T, E>> {
    let completion = rig.completion(job)?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, completion.spec.reservation_bytes);
    let JobCompletion { outcome, memory, .. } = completion;
    drop(memory);
    assert_eq!(
        rig.scope.snapshot().reserved_bytes,
        0,
        "A worker-side context kept the reservation after the terminal outcome was visible"
    );
    Ok(outcome)
}

#[test]
fn blocking_job_releases_its_reservation_once_its_terminal_is_visible() -> TestResult {
    let rig = Rig::new()?;
    for id in 1..=ROUNDS {
        let request = spec(id, JobClass::InfrastructureOrdinary, BYTES)?;
        let mut job = rig.client.submit_blocking(&rig.scope, request, |_| Ok::<_, ()>(()))?;
        assert!(matches!(released_at_terminal(&rig, &mut job)?, JobOutcome::Success(())));
    }
    Ok(())
}

#[test]
fn async_job_releases_its_reservation_once_its_terminal_is_visible() -> TestResult {
    let rig = Rig::new()?;
    for id in 1..=ROUNDS {
        let request = spec(id, JobClass::IoOrdinary, BYTES)?;
        let mut job =
            rig.client.submit_async(&rig.scope, request, |_| async { Ok::<_, ()>(()) })?;
        assert!(matches!(released_at_terminal(&rig, &mut job)?, JobOutcome::Success(())));
    }
    Ok(())
}

#[test]
fn cpu_job_releases_its_reservation_once_its_terminal_is_visible() -> TestResult {
    let rig = Rig::new()?;
    for id in 1..=ROUNDS {
        let request = spec(id, JobClass::CpuDecode, BYTES)?;
        let mut job = if id % 2 == 0 {
            rig.client.submit_cpu(&rig.scope, request, |_| YieldOnce(false))?
        } else {
            rig.client.submit_cpu(&rig.scope, request, |_| Immediate)?
        };
        assert!(matches!(released_at_terminal(&rig, &mut job)?, JobOutcome::Success(())));
    }
    Ok(())
}

#[test]
fn queued_stop_releases_its_reservation_once_its_terminal_is_visible() -> TestResult {
    let rig = Rig::with_workers(1)?;
    for round in 0..ROUNDS {
        let entered = Arc::new(Gate::default());
        let release = Arc::new(Gate::default());
        let held = Held { entered: entered.clone(), release: release.clone() };
        let mut holder = rig.client.submit_cpu(
            &rig.scope,
            spec(2 * round + 1, JobClass::CpuDecode, BYTES)?,
            move |_| held,
        )?;
        entered.wait()?;
        let mut queued = rig.client.submit_cpu(
            &rig.scope,
            spec(2 * round + 2, JobClass::CpuDecode, BYTES)?,
            |_| Immediate,
        )?;
        queued.cancel();
        release.open();
        drop(rig.completion(&mut holder)?);
        assert!(matches!(released_at_terminal(&rig, &mut queued)?, JobOutcome::Cancelled));
    }
    Ok(())
}

/// Own the job's context inside its future, then unwind past every runner.
fn unwind_owning(context: JobContext) {
    let _owned = context;
    std::panic::resume_unwind(Box::new("Supervised future unwound"));
}

#[test]
fn supervised_unwind_publishes_its_panic_without_holding_the_reservation() -> TestResult {
    let rig = Rig::new()?;
    let core = rig.client.core.clone();
    let scope_id = rig.scope.id();
    for id in 1..=ROUNDS {
        let request = spec(id, JobClass::IoOrdinary, BYTES)?;
        let weak = Arc::downgrade(&core);
        let mut job =
            core.admit(&rig.scope, request, RequestKind::Async, |context, cancel, timing| {
                let job = Job::<(), ()>::new(scope_id, request, timing.admitted, cancel, weak);
                let work =
                    Work::Async(Box::new(|context, _| Box::pin(async { unwind_owning(context) })));
                (
                    QueuedJob { context, job: job.clone(), work },
                    OwnedJob::new(scope_id, request, Box::new(JobHandle(job))),
                )
            })?;
        let completion = rig.completion(&mut job)?;
        assert!(matches!(completion.outcome, JobOutcome::Panicked(_)));
        assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
        drop(completion);
    }
    Ok(())
}
