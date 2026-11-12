// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual owner refusal and original admission cutoffs without native initialization.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::{
    configuration::{
        ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
        Theme,
    },
    coordinator_entry::EntryLaunchRequest,
    execution::{Clock, CompletionWake, ExecutionError, ExecutionPort, ScopeSpec, WaitFuture},
};
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_supervision::coordinator_entry::CoordinatorEntryInput;
type TestResult = Result<(), Box<dyn Error>>;
#[path = "claim_delivery_tests.rs"]
mod claim_delivery;

#[test]
fn observed_failure_closes_runner_admission_once_and_keeps_first_causes() -> TestResult {
    let executor = ExecutionOwner::new(Arc::new(TestClock::default()), Arc::new(Wake))?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let budget = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut authority = CoordinatorAuthority::new(budget, None)?;
    let mut entry = inert_entry(budget)?;
    let jobs = JobSequence::new();
    authority.fail(Failure::Clock(ExecutionError::new(
        Operation::Infrastructure,
        Box::new(std::io::Error::other("first_observed")),
    )));
    let batch = CompletionBatch { notifications: vec![], remaining: 0 };
    assert_eq!(
        authority.consume(&mut entry, &client, &scope, &jobs, &batch),
        Err(AuthorityIssue::Adapter)
    );
    for _ in 0..100 {
        assert_eq!(authority.consume(&mut entry, &client, &scope, &jobs, &batch), Ok(false));
    }
    assert_eq!(
        authority.prepare(&entry, &client, &scope, &jobs, r"E:\unopened".into()),
        Err(AuthorityIssue::Closed)
    );
    assert_eq!(jobs.allocate()?.get(), 1);
    authority.fail(Failure::Clock(ExecutionError::new(
        Operation::Infrastructure,
        Box::new(std::io::Error::other("first_cleanup")),
    )));
    for _ in 0..100 {
        authority.fail(Failure::Clock(ExecutionError::new(
            Operation::Infrastructure,
            Box::new(std::io::Error::other("same boundary failure")),
        )));
    }
    assert!(
        authority.failure_source().ok_or("Primary lost")?.to_string().contains("first_observed")
    );
    assert!(
        authority.cleanup_failure().ok_or("Cleanup lost")?.to_string().contains("first_cleanup")
    );
    assert_eq!(authority.suppressed_failures, 100);
    assert_eq!(authority.consume(&mut entry, &client, &scope, &jobs, &batch), Ok(false));
    Ok(())
}

#[test]
fn close_capacity_rejection_and_busy_do_not_claim_an_admitted_native_attempt() -> TestResult {
    let budget = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut authority = CoordinatorAuthority::new(budget, None)?;
    let scope = ScopeId::new(1)?;
    assert_eq!(
        authority.observe_close_admission(scope, Err(ClaimAdmissionError::Busy)),
        Err(AuthorityIssue::Pending)
    );
    assert_eq!(authority.close_attempt, CloseAttempt::Unissued);
    assert!(authority.close_job.is_none());
    assert_eq!(
        authority.observe_close_admission(scope, Err(ClaimAdmissionError::Saturated)),
        Err(AuthorityIssue::Adapter)
    );
    assert_eq!(authority.close_attempt, CloseAttempt::WaitingCapacity);
    assert!(authority.close_job.is_none());
    assert_eq!(
        authority.observe_close_admission(scope, Err(ClaimAdmissionError::Identity)),
        Err(AuthorityIssue::Adapter)
    );
    assert_eq!(authority.close_attempt, CloseAttempt::AdmissionRejected);
    assert!(authority.close_job.is_none());
    assert!(!authority.serving());
    Ok(())
}

#[derive(Default)]
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
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
fn inert_entry(budget: EntrySetupBudget) -> Result<CoordinatorEntryOwner, Box<dyn Error>> {
    let path = PathBuf::from(r"E:\authority-input");
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("Profile_1".into())?,
        Presentation::new("en", Theme::Light, false)?,
    );
    let request = EntryLaunchRequest::new(
        configuration,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        vec![0, 0],
    )?;
    Ok(CoordinatorEntryOwner::new(
        CoordinatorEntryInput::new(
            path.clone(),
            path,
            sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Account,
            request,
        )?,
        budget,
    ))
}

#[test]
fn inert_entry_cannot_prepare_claims_or_consume_job_identity() -> TestResult {
    let clock = Arc::new(TestClock::default());
    let executor = ExecutionOwner::new(clock, Arc::new(Wake))?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let budget = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut authority = CoordinatorAuthority::new(budget, None)?;
    let mut entry = inert_entry(budget)?;
    let jobs = JobSequence::new();
    assert_eq!(
        authority.prepare(&entry, &client, &scope, &jobs, PathBuf::from(r"E:\authority-input")),
        Err(AuthorityIssue::Identity)
    );
    assert_eq!(jobs.allocate()?.get(), 1);
    assert!(!authority.consume(
        &mut entry,
        &client,
        &scope,
        &jobs,
        &CompletionBatch { notifications: vec![], remaining: 0 }
    )?);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert!(!authority.serving());
    assert!(!authority.pending());
    assert!(authority.key().is_none());
    assert!(authority.registry().is_none());
    assert!(authority.registry_mut().is_none());
    assert!(authority.owners_mut().is_none());
    assert!(authority.registry_completion().is_none());
    assert!(authority.failure_source().is_none());
    assert!(authority.cleanup_failure().is_none());
    assert!(authority.stopped.is_none());
    assert!(authority.take_observation().is_none());
    assert!(authority.claims().initialization_scope().is_none());
    authority.claims_mut().close_admission();
    authority.request_close();
    authority.submit_close(&client, &scope, &jobs, budget.until())?;
    assert!(authority.settled());
    drop(authority);
    drop(entry);
    drop(scope);
    drop(executor);
    Ok(())
}

#[test]
fn original_equality_expiry_precedes_identity_allocation() -> TestResult {
    let clock = Arc::new(TestClock::default());
    let executor = ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let budget = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut authority = CoordinatorAuthority::new(budget, None)?;
    authority.association = Some((scope.owner_id(), scope.id()));
    let jobs = JobSequence::new();
    clock.0.store(budget.until().as_nanos(), Ordering::Release);
    assert_eq!(
        authority.spec(&client, &scope, &jobs, CLAIM_POOL_BYTES, budget.until()),
        Err(AuthorityIssue::Closed)
    );
    assert_eq!(jobs.allocate()?.get(), 1);
    scope.cancel();
    authority.request_close();
    assert!(authority.settled());
    drop(authority);
    drop(scope);
    drop(executor);
    Ok(())
}

#[test]
fn original_specs_use_shared_nonreused_ids_and_exact_owner_reservations() -> TestResult {
    let clock = Arc::new(TestClock::default());
    let executor = ExecutionOwner::new(clock, Arc::new(Wake))?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let budget = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut authority = CoordinatorAuthority::new(budget, None)?;
    authority.association = Some((scope.owner_id(), scope.id()));
    let jobs = JobSequence::new();
    let other = jobs.clone();
    let first = authority.spec(&client, &scope, &jobs, CLAIM_POOL_BYTES, budget.until())?;
    assert_eq!(first.reservation_bytes, CLAIM_POOL_BYTES);
    assert_eq!(other.allocate()?.get(), first.id.get() + 1);
    let second = authority.spec(
        &client,
        &scope,
        &jobs,
        WindowsProcessRegistry::required_reservation_bytes(),
        budget.until(),
    )?;
    assert_eq!(second.deadline, first.deadline);
    assert_eq!(second.id.get(), first.id.get() + 2);
    assert_eq!(second.class, JobClass::InfrastructureLifecycle);
    assert_eq!(second.operation, Operation::NativeLifecycle);
    assert!(second.replacement.is_none());
    authority.request_close();
    drop(authority);
    drop(scope);
    drop(executor);
    Ok(())
}

#[test]
fn genuine_scope_cancellation_vetoes_new_spec_before_job_identity() -> TestResult {
    let executor = ExecutionOwner::new(Arc::new(TestClock::default()), Arc::new(Wake))?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let budget = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut authority = CoordinatorAuthority::new(budget, None)?;
    authority.association = Some((scope.owner_id(), scope.id()));
    let jobs = JobSequence::new();
    scope.cancel();
    assert_eq!(
        authority.spec(&client, &scope, &jobs, CLAIM_POOL_BYTES, budget.until()),
        Err(AuthorityIssue::Closed)
    );
    assert_eq!(jobs.allocate()?.get(), 1);
    assert!(!authority.pending());
    authority.request_close();
    drop(authority);
    drop(scope);
    drop(executor);
    Ok(())
}
