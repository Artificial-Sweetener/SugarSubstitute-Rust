// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual executor and empty native registry exercise partial-admission teardown without launching processes.
use super::super::test_support::{Manual, Wake, observe};
use super::*;
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::{
    execution::{Clock, ExecutionPort, ScopeId},
    lifecycle::{CoordinatorEpoch, NamespaceId},
};
use sugarsubstitute_execution::ExecutionOwner;
mod prepared;
type TestResult = std::result::Result<(), Box<dyn Error>>;

/// Model the point before an image is supplied; all subsequent registry and execution work is real.
fn partial(client: ExecutionClient) -> Result<Parent> {
    Ok(Parent {
        work: client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        cleanup: client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        native: ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        key: CoordinatorKey {
            namespace: NamespaceId::new([5; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        client,
        retained: Some(Retained { source: None, input: None, registry: None }),
        constructing: None,
        preparing: None,
        retiring: None,
        retiring_application: false,
        process: None,
        until: None,
        wake_at: None,
        shutdown: None,
        continuation: None,
        grace_wake: None,
        settle_wake: None,
        stop_issued: false,
        retired: false,
        failure: None,
        probe: false,
        probe_custody: None,
        successful_exit: false,
        completion_taken: false,
        attempt: None,
        exit: None,
    })
}

/// Observe actual cleanup terminals before asking the sole executor to join.
fn finish(
    parent: &mut Parent,
    owner: &mut ExecutionOwner,
    wake: &Wake,
    jobs: &JobSequence,
) -> TestResult {
    observe(wake, || {
        let batch = parent.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        parent.advance(&batch, jobs).map_err(|cause| cause as Box<dyn Error>)?;
        Ok(parent.settled())
    })?;
    assert!(!parent.borrows_application());
    assert!(parent.retained.is_none());
    assert!(parent.process.is_none());
    owner.request_shutdown(parent.shutdown.ok_or("Cleanup budget absent")?.settle_until);
    observe(wake, || {
        parent.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(owner.poll_shutdown()?.settled)
    })?;
    owner.finish_shutdown()?;
    Ok(())
}

#[test]
fn close_before_admission_runs_no_registry_or_process_and_joins() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    parent.close(None).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(parent.constructing.is_none());
    finish(&mut parent, &mut owner, &wake, &JobSequence::new())
}

#[test]
fn an_attempt_that_never_reported_concludes_once_as_a_typed_failure() -> TestResult {
    use sugarsubstitute_contracts::update::attempt::{AttemptOutcome, AttemptPhase};
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    parent.close(None).map_err(|cause| cause as Box<dyn Error>)?;
    finish(&mut parent, &mut owner, &wake, &JobSequence::new())?;
    let (check, attempt) = parent.take_check_outcome().ok_or("Settled check absent")?;
    assert_eq!(check, crate::updates::schedule::CheckOutcome::Failed);
    let AttemptOutcome::Failed(report) = attempt else {
        return Err("A helper that reported nothing was not a failure".into());
    };
    assert_eq!(report.phase, AttemptPhase::Opening);
    assert!(!report.causes.is_empty());
    assert!(parent.take_check_outcome().is_none());
    Ok(())
}

#[test]
fn normal_application_close_before_admission_never_starts_a_helper() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let budget = ShutdownBudget::new(clock.now())?;
    parent.application_closing(Some(budget), true).map_err(|cause| cause as Box<dyn Error>)?;
    assert_eq!(parent.shutdown, Some(budget));
    assert!(parent.post_exit_limit().is_none());
    assert!(parent.constructing.is_none());
    assert!(parent.process.is_none());
    finish(&mut parent, &mut owner, &wake, &JobSequence::new())
}

#[test]
fn normal_application_close_during_partial_admission_keeps_original_cleanup() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let jobs = JobSequence::new();
    parent.launch(&jobs).map_err(|cause| cause as Box<dyn Error>)?;
    let budget = ShutdownBudget::new(clock.now())?;
    parent.application_closing(Some(budget), true).map_err(|cause| cause as Box<dyn Error>)?;
    assert_eq!(parent.shutdown, Some(budget));
    assert!(parent.post_exit_limit().is_none());
    assert!(parent.constructing.is_some());
    assert!(parent.process.is_none());
    finish(&mut parent, &mut owner, &wake, &jobs)
}

#[test]
fn partially_admitted_registry_is_observed_and_released_before_join() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let jobs = JobSequence::new();
    parent.launch(&jobs).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(parent.constructing.is_some());
    assert!(parent.process.is_none());
    parent.close(None).map_err(|cause| cause as Box<dyn Error>)?;
    finish(&mut parent, &mut owner, &wake, &jobs)
}

#[test]
fn a_running_preparation_helper_has_no_overall_cutoff_or_campaign_wake() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let jobs = JobSequence::new();
    assert_eq!(parent.next_deadline(), None);
    // Hours beyond the former fixed campaign, an open helper is still not closed by time.
    clock.0.store(3 * 60 * 60 * 1_000_000_000, Ordering::SeqCst);
    let batch = parent.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
    parent.advance(&batch, &jobs).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(parent.shutdown.is_none());
    assert!(parent.failure.is_none());
    assert_eq!(parent.next_deadline(), None);
    let deadline = parent
        .constructing
        .as_ref()
        .ok_or("Registry construction was not admitted")?
        .spec()
        .deadline
        .ok_or("Launch step has no deadline")?;
    assert_eq!(deadline, clock.now().checked_add(super::super::STEP)?);
    parent.close(None).map_err(|cause| cause as Box<dyn Error>)?;
    finish(&mut parent, &mut owner, &wake, &jobs)
}

#[test]
fn image_failure_closes_only_preparation_and_retains_the_original_cause() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let jobs = JobSequence::new();
    let spec = parent
        .spec(
            &jobs,
            JobClass::InfrastructureLifecycle,
            4096,
            Some(parent.step_deadline().map_err(|cause| cause as Box<dyn Error>)?),
        )
        .map_err(|cause| cause as Box<dyn Error>)?;
    parent.preparing = Some(parent.client.submit_blocking(&parent.work, spec, |_| {
        Err(io::Error::other("verified image unavailable"))
    })?);
    assert!(parent.borrows_application());
    finish(&mut parent, &mut owner, &wake, &jobs)?;
    assert_eq!(
        parent.failure.as_ref().ok_or("Original failure absent")?.to_string(),
        "verified image unavailable"
    );
    Ok(())
}

#[test]
fn repeated_close_preserves_the_application_budget_and_outer_cutoff() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let campaign = parent.until;
    let budget = ShutdownBudget::new(clock.now())?;
    parent.close(Some(budget)).map_err(|cause| cause as Box<dyn Error>)?;
    // Compare the retained wake instants: `next_deadline` also filters them by real elapsed time.
    let wakes = (parent.grace_wake, parent.settle_wake);
    clock.0.store(1_000_000_000, Ordering::SeqCst);
    parent
        .close(Some(ShutdownBudget::new(clock.now())?))
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert_eq!(parent.shutdown, Some(budget));
    assert_eq!(parent.until, campaign);
    assert_eq!((parent.grace_wake, parent.settle_wake), wakes);
    finish(&mut parent, &mut owner, &wake, &JobSequence::new())
}

#[test]
fn expired_cleanup_preserves_unsettled_custody_without_admission_or_deadline_renewal() -> TestResult
{
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let mut parent = partial(client.clone()).map_err(|cause| cause as Box<dyn Error>)?;
    parent.close(None).map_err(|cause| cause as Box<dyn Error>)?;
    let original = parent.shutdown;
    clock.0.store(16_000_000_000, Ordering::SeqCst);
    let jobs = JobSequence::new();
    let batch = CompletionBatch { notifications: vec![], remaining: 0 };
    parent.advance(&batch, &jobs).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(!parent.settled());
    assert!(parent.retained.is_some());
    assert!(parent.retiring.is_none());
    assert_eq!(
        parent
            .failure
            .as_ref()
            .and_then(|cause| cause.downcast_ref::<io::Error>())
            .map(io::Error::kind),
        Some(io::ErrorKind::TimedOut)
    );
    parent.advance(&batch, &jobs).map_err(|cause| cause as Box<dyn Error>)?;
    assert_eq!(parent.shutdown, original);
    assert!(parent.next_deadline().is_none());
    assert_eq!(jobs.allocate()?.get(), 1);
    drop(parent);
    owner.request_shutdown(ShutdownBudget::new(clock.now())?.settle_until);
    observe(&wake, || {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(owner.poll_shutdown()?.settled)
    })?;
    owner.finish_shutdown()?;
    Ok(())
}
