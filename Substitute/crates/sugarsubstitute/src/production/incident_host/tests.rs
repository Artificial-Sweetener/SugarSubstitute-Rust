// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real original reservations through terminal transfer, worker join and final Drop.
use super::*;
use crate::completion_wake::Wake;
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_execution::ExecutionOwner;

#[path = "tests/constructor.rs"]
pub(in crate::production) mod constructor;

#[test]
fn deferred_report_record_fits_real_original_policy_admission_until_checked_disposal()
-> Result<(), Box<dyn Error + Send + Sync>> {
    admitted_value(CONTROLLER_BYTES, |context| {
        let owner = CoordinatorKey {
            namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
            epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([2; 16])?,
        };
        let policy = super::admission::allocate_policy(owner, &context)?;
        let peak = IncidentCoordinator::required_storage_bytes()
            .checked_add(size_of::<PolicyState>() as u64)
            .ok_or("Original policy peak overflow")?;
        assert!(peak <= POLICY_BYTES);
        assert_eq!(context.spec().reservation_bytes, CONTROLLER_BYTES);
        Ok(policy)
    })
}

#[derive(Default)]
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
fn key() -> Result<CoordinatorKey, Box<dyn Error>> {
    Ok(CoordinatorKey {
        namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
        epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([2; 16])?,
    })
}
fn spec(id: JobId) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: CONTROLLER_BYTES,
        replacement: None,
    }
}
#[test]
fn controller_table_retains_original_charge_after_terminal_and_actual_worker_join()
-> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let owner = key()?;
    let jobs = JobSequence::new();
    let mut job = client.submit_blocking(&scope, spec(jobs.allocate()?), move |context| {
        super::admission::allocate_policy(owner, &context)
    })?;
    let JobCompletion { outcome, memory, .. } = wake.completion(&client, &mut job)?;
    let policy = match outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("Real controller allocation did not succeed".into()),
    };
    assert_eq!(policy.state.as_ref().ok_or("no funded state")?._claim.bytes(), POLICY_BYTES);
    assert!(
        IncidentCoordinator::required_storage_bytes() + (size_of::<PolicyState>() as u64)
            < policy.state.as_ref().ok_or("no funded state")?._claim.bytes()
    );
    drop(memory);
    drop(job);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    assert!(
        policy.state.as_ref().ok_or("no funded state")?.state.effects.iter().all(Option::is_none)
    );
    drop(policy);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn exhausted_original_claim_preserves_genuine_memory_cause_before_any_controller_allocation()
-> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let owner = key()?;
    let jobs = JobSequence::new();
    let mut job = client.submit_blocking(&scope, spec(jobs.allocate()?), move |context| {
        let held = context.memory().claim(POLICY_BYTES)?;
        let attempted = super::admission::allocate_policy(owner, &context);
        Ok::<_, MemoryClaimError>((attempted, held))
    })?;
    let JobCompletion { outcome, memory, .. } = wake.completion(&client, &mut job)?;
    let (attempted, held) = match outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("Original exhaustion case did not complete".into()),
    };
    let cause = match attempted {
        Err(cause) => cause,
        Ok(_) => return Err("Unexpected second allocation succeeded".into()),
    };
    let original = cause
        .source()
        .and_then(|source| source.downcast_ref::<MemoryClaimError>())
        .ok_or("Original memory cause lost")?;
    assert_eq!(original.requested_bytes, POLICY_BYTES);
    assert_eq!(original.available_bytes, CONTROLLER_BYTES - POLICY_BYTES);
    assert_eq!(cause.to_string(), "Incident controller capacity rejected");
    drop(memory);
    drop(job);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(cause);
    drop(held);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn closing_keeps_the_original_earliest_cutoff_without_claiming_native_settlement()
-> Result<(), Box<dyn Error>> {
    let mut host = IncidentHost::new();
    let first = ShutdownBudget::new(Moment::from_nanos(5))?;
    host.request_close(first);
    host.request_close(ShutdownBudget::new(Moment::from_nanos(10))?);
    assert_eq!(host.shutdown, Some(first));
    assert!(matches!(host.state(), Err(HostIssue::Closed)));
    assert!(host.construction_settled());
    Ok(())
}

#[test]
fn missing_collection_input_retains_the_identical_funded_policy_before_job_admission()
-> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let owner = key()?;
    let jobs = JobSequence::new();
    let mut job = client.submit_blocking(&scope, spec(jobs.allocate()?), move |context| {
        super::admission::allocate_policy(owner, &context)
    })?;
    let done = wake.completion(&client, &mut job)?;
    let mut policy = match done.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("No actual controller".into()),
    };
    let before = policy.state.as_ref().map(|state| std::ptr::from_ref(&*state.state));
    let mut input = None;
    assert_eq!(
        policy.collection.submit(
            &client,
            &scope,
            &jobs,
            Moment::from_nanos(30_000_000_000),
            &mut input,
            &mut policy.state
        ),
        Err(super::super::diagnostic_helper::collection::CollectionIssue::Identity)
    );
    assert_eq!(policy.state.as_ref().map(|state| std::ptr::from_ref(&*state.state)), before);
    assert!(input.is_none());
    drop(done.memory);
    drop(job);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(policy);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[cfg(test)]
pub(in crate::production) fn admitted_value<T: Send + 'static>(
    bytes: u64,
    work: impl FnOnce(JobContext) -> Result<T, Box<dyn Error + Send + Sync>> + Send + 'static,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut specification = spec(JobId::new(1)?);
    specification.reservation_bytes = bytes;
    specification.deadline = Some(Moment::from_nanos(100));
    let mut job = client.submit_blocking(&scope, specification, work)?;
    let done = wake
        .completion(&client, &mut job)
        .map_err(|cause| std::io::Error::other(cause.to_string()))?;
    let text = match done.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("No real charged worker outcome".into()),
    };
    drop(done.memory);
    drop(job);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, bytes);
    drop(text);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Transfer the same genuinely admitted controller state into close-only owner tests.
pub(in crate::production) fn collection_policy_fixture(
    context: &JobContext,
) -> Result<FundedState, Box<dyn Error + Send + Sync>> {
    let owner = CoordinatorKey {
        namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
        epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([2; 16])?,
    };
    let mut controller = super::admission::allocate_policy(owner, context)?;
    controller.state.take().ok_or_else(|| "Actual funded policy missing".into())
}
