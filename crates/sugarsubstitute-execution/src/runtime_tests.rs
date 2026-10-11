// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify actual scheduler admission and retained terminal lifetimes.

use crate::runtime_test_support::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::*;

struct ReentrantCancellationWake {
    client: crate::runtime::ExecutionClient,
    scope_spec: ScopeSpec,
    observed: Arc<Gate>,
}
impl std::task::Wake for ReentrantCancellationWake {
    fn wake(self: Arc<Self>) {
        assert!(matches!(
            self.client.create_scope(self.scope_spec, None),
            Err(AdmissionError::ShuttingDown)
        ));
        self.observed.open();
    }
}

#[test]
fn shutdown_notifies_public_cancel_waiters_outside_the_admission_lock() -> TestResult {
    let mut rig = Rig::new()?;
    let mut job = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::IoOrdinary, 0)?,
        |context| async { Ok::<_, ()>(context) },
    )?;
    let completion = rig.completion(&mut job)?;
    let context = match completion.outcome {
        JobOutcome::Success(context) => context,
        _ => return Err(std::io::Error::other("Expected retained context").into()),
    };
    let mut cancelled = context.cancelled();
    let observed = Arc::new(Gate::default());
    let waker = std::task::Waker::from(Arc::new(ReentrantCancellationWake {
        client: rig.client.clone(),
        scope_spec: ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        observed: observed.clone(),
    }));
    let mut poll_context = std::task::Context::from_waker(&waker);
    assert!(cancelled.as_mut().poll(&mut poll_context).is_pending());
    rig.owner.request_shutdown(Moment::from_nanos(1));
    observed.wait()?;
    assert!(cancelled.as_mut().poll(&mut poll_context).is_ready());
    Ok(())
}

#[test]
fn dropped_child_remains_live_until_work_and_retained_completion_are_released() -> TestResult {
    let rig = Rig::new()?;
    let child = rig
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&rig.scope))?;
    let started = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let worker_started = started.clone();
    let worker_release = release.clone();
    let mut job = rig.client.submit_blocking(
        &child,
        spec(1, JobClass::InfrastructureOrdinary, 1)?,
        move |_| {
            worker_started.open();
            worker_release.wait().map_err(|error| error.to_string())
        },
    )?;
    started.wait()?;
    drop(child);
    assert_eq!(rig.scope.snapshot().children_live, 1);
    release.open();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().children_live, 1);
    drop(completion);
    assert_eq!(rig.scope.snapshot().children_live, 0);
    Ok(())
}

#[test]
fn accepted_result_retains_peak_bytes_after_credit_acknowledgement() -> TestResult {
    let rig = Rig::new()?;
    let mut job = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::IoOrdinary, 256 * 1024 * 1024)?,
        |context| async move { Ok::<_, ()>(("pixels", context.memory().clone())) },
    )?;
    let completion = rig.completion(&mut job)?;
    assert_eq!(rig.scope.snapshot().terminal_unobserved, 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 256 * 1024 * 1024);
    let called = Arc::new(AtomicBool::new(false));
    let called_factory = called.clone();
    let rejected =
        rig.client.submit_async(&rig.scope, spec(2, JobClass::IoOrdinary, 1)?, move |_| {
            called_factory.store(true, Ordering::Release);
            async { Ok::<_, ()>(()) }
        });
    assert!(matches!(rejected, Err(AdmissionError::MemorySaturated { available_bytes: 0, .. })));
    assert!(!called.load(Ordering::Acquire));
    let JobCompletion { outcome, memory, .. } = completion;
    drop(memory);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 256 * 1024 * 1024);
    drop(outcome);
    // Real worker/context termination may still own its lease until the supervisor
    // observes the join; teardown establishes that every such owner has stopped.
    drop(job);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn blocking_cancellation_does_not_claim_stop_before_external_work_returns() -> TestResult {
    let mut rig = Rig::new()?;
    let started = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let worker_started = started.clone();
    let worker_release = release.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 10)?,
        move |_| {
            worker_started.open();
            worker_release.wait().map_err(|error| error.to_string())?;
            Ok::<_, String>(7)
        },
    )?;
    started.wait()?;
    job.cancel();
    assert!(job.try_take_completion().is_none());
    assert_eq!(job.status().phase, JobPhase::Running);
    assert!(job.status().cancellation_requested);
    rig.owner.request_shutdown(Moment::from_nanos(5));
    rig.clock.advance(5);
    let progress = rig.owner.poll_shutdown()?;
    assert!(!progress.settled);
    assert!(progress.deadline_expired);
    assert_eq!(progress.unresolved_job_ids, vec![JobId::new(1)?]);
    assert!(rig.owner.finish_shutdown().is_err());
    release.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    drop(rig.owner);
    Ok(())
}

#[test]
fn scope_parent_deadline_and_drop_cancel_the_owned_async_future() -> TestResult {
    let rig = Rig::new()?;
    let parent = rig.client.create_scope(
        ScopeSpec { id: ScopeId::new(2)?, deadline: Some(Moment::from_nanos(100)) },
        None,
    )?;
    let child = rig.client.create_scope(
        ScopeSpec { id: ScopeId::new(3)?, deadline: Some(Moment::from_nanos(200)) },
        Some(&parent),
    )?;
    let started = Arc::new(Gate::default());
    let started_factory = started.clone();
    let mut job = rig.client.submit_async(
        &child,
        spec(1, JobClass::IoLifecycle, 0)?,
        move |context| async move {
            assert_eq!(context.deadline(), Some(Moment::from_nanos(100)));
            started_factory.open();
            std::future::pending::<Result<(), ()>>().await
        },
    )?;
    started.wait()?;
    rig.clock.advance(100);
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    drop(parent);
    assert!(child.snapshot().cancellation_requested);
    assert!(matches!(
        rig.client.submit_async(&child, spec(2, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::ScopeClosed { .. })
    ));
    Ok(())
}

#[test]
fn foreign_scope_and_wrong_method_reject_before_factory_and_do_not_advance_ids() -> TestResult {
    let rig = Rig::new()?;
    let foreign = Rig::new()?;
    let result =
        rig.client.submit_async(&foreign.scope, spec(1, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        });
    assert!(matches!(result, Err(AdmissionError::ForeignScope { .. })));
    let result = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 0)?,
        |_| async { Ok::<_, ()>(()) },
    );
    assert!(matches!(result, Err(AdmissionError::WrongClass { .. })));
    let mut job =
        rig.client.submit_async(&rig.scope, spec(1, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        })?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Success(())));
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(1, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::JobIdentityOrder { .. })
    ));
    Ok(())
}

#[path = "runtime_notification_tests.rs"]
mod notifications;
