// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove panic and cancellation outcomes follow actual owned destruction.

use crate::runtime_test_support::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
};
use sugarsubstitute_contracts::execution::*;

struct DropPanic(Arc<AtomicBool>);
impl Drop for DropPanic {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
        std::panic::resume_unwind(Box::new("Owned destructor failed".to_owned()));
    }
}

struct PendingWithDrop {
    started: Arc<Gate>,
    bomb: DropPanic,
}
impl Future for PendingWithDrop {
    type Output = Result<(), ()>;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let _retained = &self.bomb;
        self.started.open();
        Poll::Pending
    }
}

#[test]
fn async_factory_and_poll_panics_are_owned_and_do_not_stop_the_adapter() -> TestResult {
    let rig = Rig::new()?;
    let mut factory_panic = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::IoOrdinary, 0)?,
        |_| -> std::future::Ready<Result<(), ()>> {
            std::panic::resume_unwind(Box::new("Factory failure".to_owned()))
        },
    )?;
    let completion = rig.completion(&mut factory_panic)?;
    assert!(
        matches!(completion.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "Factory failure")
    );
    let mut poll_panic =
        rig.client.submit_async(&rig.scope, spec(2, JobClass::IoOrdinary, 0)?, |_| async {
            std::panic::resume_unwind(Box::new("糖".repeat(400)));
            #[allow(unreachable_code)]
            Ok::<_, ()>(())
        })?;
    assert!(
        matches!(rig.completion(&mut poll_panic)?.outcome, JobOutcome::Panicked(cause) if cause.as_str().len() == 1023)
    );
    let mut next =
        rig.client.submit_async(&rig.scope, spec(3, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(9)
        })?;
    assert!(matches!(rig.completion(&mut next)?.outcome, JobOutcome::Success(9)));
    Ok(())
}

#[test]
fn async_cancel_observes_future_destructor_panic_before_terminal() -> TestResult {
    let rig = Rig::new()?;
    let started = Arc::new(Gate::default());
    let destroyed = Arc::new(AtomicBool::new(false));
    let worker_started = started.clone();
    let worker_destroyed = destroyed.clone();
    let mut job =
        rig.client.submit_async(&rig.scope, spec(1, JobClass::IoOrdinary, 0)?, move |_| {
            PendingWithDrop { started: worker_started, bomb: DropPanic(worker_destroyed) }
        })?;
    started.wait()?;
    job.cancel();
    let completion = rig.completion(&mut job)?;
    assert!(destroyed.load(Ordering::Acquire));
    assert!(
        matches!(completion.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "Owned destructor failed")
    );
    Ok(())
}

#[test]
fn cancelling_pending_work_destroys_factory_without_calling_it() -> TestResult {
    let rig = Rig::new()?;
    let release = Arc::new(Gate::default());
    let mut running = Vec::new();
    for id in 1..=8 {
        let started = Arc::new(Gate::default());
        let worker_started = started.clone();
        let worker_release = release.clone();
        running.push(rig.client.submit_async(
            &rig.scope,
            spec(id, JobClass::IoOrdinary, 0)?,
            move |_| async move {
                worker_started.open();
                worker_release.future().await;
                Ok::<_, ()>(())
            },
        )?);
        started.wait()?;
    }
    let called = Arc::new(AtomicBool::new(false));
    let destroyed = Arc::new(AtomicBool::new(false));
    let bomb = DropPanic(destroyed.clone());
    let factory_called = called.clone();
    let mut queued =
        rig.client.submit_async(&rig.scope, spec(9, JobClass::IoOrdinary, 0)?, move |_| {
            let _owned = bomb;
            factory_called.store(true, Ordering::Release);
            async { Ok::<_, ()>(()) }
        })?;
    assert_eq!(queued.status().phase, JobPhase::Queued);
    queued.cancel();
    let completion = rig.completion(&mut queued)?;
    assert!(!called.load(Ordering::Acquire));
    assert!(destroyed.load(Ordering::Acquire));
    assert!(matches!(completion.outcome, JobOutcome::Panicked(_)));
    assert_eq!(completion.execution_time, std::time::Duration::ZERO);
    release.open();
    for job in &mut running {
        assert!(matches!(rig.completion(job)?.outcome, JobOutcome::Success(())));
    }
    Ok(())
}

#[test]
fn blocking_error_and_panic_keep_their_typed_owned_outcomes() -> TestResult {
    let rig = Rig::new()?;
    let mut failure = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 0)?,
        |_| Err::<(), _>("Exact domain failure"),
    )?;
    assert!(matches!(
        rig.completion(&mut failure)?.outcome,
        JobOutcome::Failure("Exact domain failure")
    ));
    let mut panic = rig.client.submit_blocking(
        &rig.scope,
        spec(2, JobClass::InfrastructureOrdinary, 0)?,
        |_| -> Result<(), ()> {
            std::panic::resume_unwind(Box::new("Finite worker failure".to_owned()))
        },
    )?;
    assert!(
        matches!(rig.completion(&mut panic)?.outcome, JobOutcome::Panicked(cause) if cause.as_str() == "Finite worker failure")
    );
    Ok(())
}

#[test]
fn abandoned_job_keeps_real_blocking_join_until_worker_returns() -> TestResult {
    let mut rig = Rig::new()?;
    let started = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let worker_started = started.clone();
    let worker_release = release.clone();
    let job = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 7)?,
        move |_| {
            worker_started.open();
            worker_release.wait().map_err(|error| error.to_string())
        },
    )?;
    started.wait()?;
    drop(job);
    assert_eq!(rig.scope.snapshot().running, 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 7);
    rig.owner.request_shutdown(Moment::from_nanos(1));
    assert_eq!(rig.owner.poll_shutdown()?.unresolved_job_ids, vec![JobId::new(1)?]);
    release.open();
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().running, 0);
    assert_eq!(rig.scope.snapshot().terminal_unobserved, 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[path = "task_disposal_tests.rs"]
mod disposal;
