// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reenter actual capture controls from external clock observations without mutex contention gates.
use super::{test_support::*, *};
use crate::runtime_test_support::{Gate, Rig, TestResult};
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::*;
type RetainedShared = Arc<Mutex<Option<Arc<Shared>>>>;
fn arm(
    rig: &Rig,
    boundary: &Boundary,
    action: Box<dyn FnOnce(Arc<Shared>) + Send>,
) -> (Arc<Gate>, RetainedShared) {
    let arrived = Arc::new(Gate::default());
    let done = arrived.clone();
    let retained = Arc::new(Mutex::new(None));
    let stored = retained.clone();
    let clock = rig.clock.clone();
    *lock(&boundary.clock_probe) = Some(Box::new(move |shared| {
        let shared = shared.clone();
        *lock(&stored) = Some(shared.clone());
        clock.on_next_current_thread_observation(Box::new(move || {
            action(shared);
            done.open();
        }));
    }));
    (arrived, retained)
}
fn completion(
    rig: &Rig,
    job: &mut OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
) -> TestResult<(JobCompletion<CaptureAttachLaunchPin, CaptureAttachFailure>, Vec<ExecutionError>)>
{
    let mut causes = Vec::new();
    loop {
        let observed = rig.wake.count();
        while let Err(cause) = rig.client.drain_completions(NonZeroUsize::MIN) {
            causes.push(cause);
        }
        if let Some(completion) = job.try_take_completion() {
            return Ok((completion, causes));
        }
        rig.wake.wait_after(observed)?;
    }
}
fn retained_before_close(
    rig: &Rig,
    scope: &Scope,
    boundary: &Boundary,
    job: &mut OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
) -> TestResult {
    boundary.retiring.wait()?;
    assert_eq!(boundary.starts.load(Ordering::Acquire), 0);
    assert_eq!(boundary.observations.load(Ordering::Acquire), 1);
    assert_eq!(scope.snapshot().reserved_bytes, 16_384);
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert!(!boundary.physical_closed.load(Ordering::Acquire));
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 0);
    assert!(job.try_take_completion().is_none());
    assert!(
        lock(&rig.client.core.state)
            .jobs
            .get(&job.spec().id)
            .is_some_and(|record| record.held_native())
    );
    Ok(())
}
#[test]
fn actual_provider_scope_cancel_vetoes_capture_issue_and_keeps_custody_until_checked_close()
-> TestResult {
    let rig = Rig::new()?;
    let scope = Arc::new(scope(&rig)?);
    let boundary = Arc::new(Boundary::default());
    let _release = ReleaseBoundary::new(&rig.client.core, &boundary);
    let cancelling = scope.clone();
    let (arrived, shared) = arm(&rig, &boundary, Box::new(move |_| cancelling.cancel()));
    let mut job = submit(&rig, &scope, &boundary)?;
    arrived.wait()?;
    retained_before_close(&rig, &scope, &boundary, &mut job)?;
    let shared = lock(&shared).clone().ok_or("Actual shared custody was lost")?;
    let CaptureCleanupSnapshot::Latched { budget, .. } = shared.custody.snapshot() else {
        return Err("Original scope cleanup did not latch".into());
    };
    assert_eq!(budget.graceful_until, Moment::from_nanos(10_000_000_000));
    assert_eq!(budget.settle_until, Moment::from_nanos(15_000_000_000));
    boundary.settle()?;
    let (completion, causes) = completion(&rig, &mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert!(causes.is_empty());
    drop(completion);
    boundary.source_retired.wait()?;
    assert!(
        lock(&shared.custody.state).native_retired && lock(&shared.custody.state).source_retired
    );
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn actual_provider_cleanup_request_keeps_one_original_budget_and_never_publishes_a_pin()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release = ReleaseBoundary::new(&rig.client.core, &boundary);
    let (arrived, shared) = arm(
        &rig,
        &boundary,
        Box::new(move |shared| {
            let control = control::CaptureCleanupControl { custody: shared.custody.clone() };
            control.request_cleanup();
        }),
    );
    let mut job = submit(&rig, &scope, &boundary)?;
    arrived.wait()?;
    retained_before_close(&rig, &scope, &boundary, &mut job)?;
    let shared = lock(&shared).clone().ok_or("Actual shared custody was lost")?;
    let CaptureCleanupSnapshot::Latched { budget: first, .. } = shared.custody.snapshot() else {
        return Err("First actual cleanup was not latched".into());
    };
    rig.clock.advance(1_000_000_000);
    let control = control::CaptureCleanupControl { custody: shared.custody.clone() };
    let CaptureCleanupSnapshot::Latched { budget: retained, .. } = control.request_cleanup() else {
        return Err("Repeated actual cleanup was not retained".into());
    };
    assert_eq!(retained, first);
    boundary.settle()?;
    let (completion, causes) = completion(&rig, &mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert!(causes.is_empty());
    Ok(())
}
#[test]
fn actual_provider_original_attach_equality_expires_without_issue_or_budget_renewal() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release = ReleaseBoundary::new(&rig.client.core, &boundary);
    let clock = rig.clock.clone();
    let (arrived, shared) = arm(&rig, &boundary, Box::new(move |_| clock.advance(60_000_000_000)));
    let mut job = submit(&rig, &scope, &boundary)?;
    arrived.wait()?;
    retained_before_close(&rig, &scope, &boundary, &mut job)?;
    let shared = lock(&shared).clone().ok_or("Actual shared custody was lost")?;
    let CaptureCleanupSnapshot::Latched { budget, .. } = shared.custody.snapshot() else {
        return Err("Original timeout cleanup was not latched".into());
    };
    assert_eq!(budget.graceful_until, Moment::from_nanos(70_000_000_000));
    assert_eq!(budget.settle_until, Moment::from_nanos(75_000_000_000));
    boundary.settle()?;
    let (completion, causes) = completion(&rig, &mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::TimedOut));
    assert!(causes.is_empty());
    assert_eq!(shared.custody.expected.attach_budget().until(), Moment::from_nanos(60_000_000_000));
    Ok(())
}
#[test]
fn actual_provider_panic_preserves_first_cause_and_prepared_custody_before_real_retirement()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release = ReleaseBoundary::new(&rig.client.core, &boundary);
    let clock = rig.clock.clone();
    *lock(&boundary.clock_probe) = Some(Box::new(move |_| {
        clock.on_next_current_thread_observation(Box::new(|| {
            std::panic::resume_unwind(Box::new("Capture issue provider original panic"));
        }));
    }));
    let mut job = submit(&rig, &scope, &boundary)?;
    retained_before_close(&rig, &scope, &boundary, &mut job)?;
    assert!(rig.client.core.clock.failed());
    let cause = rig.client.core.clock.metadata().err().ok_or("Fatal clock cause was lost")?;
    assert_eq!(
        std::error::Error::source(&cause).ok_or("Original cause not retained")?.to_string(),
        "Capture issue provider original panic"
    );
    boundary.settle()?;
    let (completion, causes) = completion(&rig, &mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Panicked(ref cause)
        if cause.as_str()=="Capture issue provider original panic"));
    assert!(!causes.is_empty());
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn actual_foreign_same_number_context_cannot_issue_capture_work_under_another_owner() -> TestResult
{
    let alien = Rig::new()?;
    let (_alien_scope, context) = crate::runtime_test_support::admitted_context(&alien, 2, 1)?;
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    assert_eq!(scope.id(), context.scope_id());
    assert_ne!(scope.owner_id(), context.owner_id());
    let boundary = Arc::new(Boundary::default());
    let _release = ReleaseBoundary::new(&rig.client.core, &boundary);
    *lock(&boundary.context_override) = Some(context);
    let mut job = submit(&rig, &scope, &boundary)?;
    retained_before_close(&rig, &scope, &boundary, &mut job)?;
    boundary.settle()?;
    let (completion, causes) = completion(&rig, &mut job)?;
    assert!(
        matches!(completion.outcome, JobOutcome::Failure(CaptureAttachFailure::Native(ref cause))
        if cause.stage()=="capture_issue_context")
    );
    assert!(!causes.is_empty());
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 1);
    Ok(())
}
