// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original real scope custody and no-effect rejection before native admission.
use super::super::test_support::InertDriver;
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{
    execution::{Clock, CompletionWake, ExecutionError, ExecutionPort, ScopeId, WaitFuture},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};
use sugarsubstitute_execution::{
    ExecutionOwner,
    windows_lifecycle_pipe::{InheritedLifecycleBootstrap, LifecyclePipeDeadline},
};
type TestResult = Result<(), Box<dyn Error>>;
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}
fn runtime() -> Result<ExecutionOwner, Box<dyn Error>> {
    Ok(ExecutionOwner::new(Arc::new(FrozenClock), Arc::new(Wake))?)
}
fn bootstrap(
    calls: &Arc<AtomicUsize>,
    deadline: Option<Moment>,
) -> Result<ChildBootstrap, Box<dyn Error>> {
    let expected = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let channel = ChildChannel::new(
        InheritedLifecycleBootstrap::new(1)?,
        LifecyclePipeDeadline::new(100, 100)?,
        expected,
    );
    Ok(ChildBootstrap::new(
        channel,
        ChildCapture::new(Box::new(InertDriver(calls.clone()))),
        ScopeSpec { id: ScopeId::new(3)?, deadline },
        (LaunchId::new(1)?, Generation::new(1)?),
    )?)
}
#[test]
fn cancelled_original_scope_rejects_before_independent_native_scope_and_job_identity() -> TestResult
{
    let calls = Arc::new(AtomicUsize::new(0));
    let owner = runtime()?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    scope.cancel();
    let mut child = bootstrap(&calls, None)?;
    let jobs = JobSequence::new();
    assert_eq!(child.start(&client, scope, &jobs), Err(ChildBootstrapIssue::State));
    assert!(child.scope.as_ref().ok_or("Original scope lost")?.snapshot().cancellation_requested);
    assert!(child.native_scope.is_none());
    assert_eq!(jobs.allocate()?.get(), 1);
    assert!(!child.channel.registered());
    assert!(child.take_prepared_startup().is_none());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    drop(child);
    drop(owner);
    Ok(())
}
#[test]
fn close_before_start_retains_actual_scope_without_native_work_or_cleanup_budget() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    let owner = runtime()?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut child = bootstrap(&calls, None)?;
    child.close_initialization();
    assert_eq!(child.start(&client, scope, &JobSequence::new()), Err(ChildBootstrapIssue::State));
    assert!(child.scope.is_some());
    assert!(child.native_scope.is_none());
    assert!(child.shutdown_budget().is_none());
    assert!(child.scope().is_some());
    assert!(child.native_scope().is_none());
    assert!(child.original_deadline().is_none());
    assert!(!child.projection_pending());
    assert!(!child.capture_admitted());
    assert!(!child.capture_pending());
    assert!(child.capture_cleanup_snapshot().is_none());
    assert!(child.try_take_process_lifetime().is_none());
    assert!(child.channel_mut().received_header()?.is_none());
    assert!(!child.channel_mut().admitted());
    child.release_source_after_native_retired()?;
    assert!(child.failure_source().is_none());
    assert!(child.take_prepared_startup().is_none());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    drop(child);
    drop(owner);
    Ok(())
}
#[test]
fn persistent_channel_deadline_is_rejected_inertly() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    assert!(bootstrap(&calls, Some(Moment::from_nanos(100))).is_err());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    Ok(())
}
