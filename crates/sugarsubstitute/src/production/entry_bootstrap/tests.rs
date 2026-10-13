// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real preparation admission/completion while faking only clock and wake.
use super::*;
#[path = "test_support.rs"]
mod support;
use sugarsubstitute_contracts::execution::{ExecutionPort, ScopeId, ScopeSpec};
use support::{Files, Runtime};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn inert_bootstrap_has_no_native_owner_or_original_execution_scope() -> TestResult {
    let files = Files::new()?;
    let mut bootstrap = files.bootstrap()?;
    assert!(!bootstrap.started);
    assert!(bootstrap.scope.is_none());
    assert!(bootstrap.owner().is_none());
    assert!(bootstrap.owner_mut().is_none());
    assert!(bootstrap.failure_source().is_none());
    assert!(bootstrap.cleanup_failure().is_none());
    assert!(bootstrap.failed_preparation().is_none());
    drop(bootstrap);
    files.finish()?;
    Ok(())
}

#[test]
fn real_configuration_failure_retains_original_scope_completion_and_cause() -> TestResult {
    let files = Files::new()?;
    let runtime = Runtime::new()?;
    let scope = runtime.scope(1)?;
    let mut bootstrap = files.bootstrap()?;
    let jobs = JobSequence::new();
    bootstrap.start(&runtime.client, scope, &jobs)?;
    assert_eq!(runtime.failure(&mut bootstrap, &jobs)?, EntryBootstrapIssue::Preparation);
    assert!(bootstrap.owner().is_none());
    assert!(bootstrap.failure_source().is_some());
    let original = bootstrap.failed_preparation().ok_or("Missing original preparation")?;
    assert_eq!(original.scope_id, bootstrap.scope.as_ref().ok_or("Scope lost")?.id());
    assert_eq!(original.spec.deadline, Some(bootstrap.original.until()));
    assert_eq!(original.outcome.kind(), TerminalKind::Failure);
    assert_eq!(
        bootstrap.scope.as_ref().ok_or("Scope lost")?.snapshot().reserved_bytes,
        configuration::CONFIGURATION_RESERVATION_BYTES
    );
    assert!(bootstrap.scope.as_ref().ok_or("Scope lost")?.snapshot().terminal_unobserved == 0);
    drop(bootstrap);
    drop(runtime);
    files.finish()?;
    Ok(())
}

#[test]
fn cancelled_actual_scope_rejects_before_configuration_and_keeps_original_source_scope()
-> TestResult {
    let files = Files::new()?;
    let runtime = Runtime::new()?;
    let scope = runtime.scope(1)?;
    scope.cancel();
    let mut bootstrap = files.bootstrap()?;
    assert_eq!(
        bootstrap.start(&runtime.client, scope, &JobSequence::new()),
        Err(EntryBootstrapIssue::Preparation)
    );
    assert!(bootstrap.preparation.is_none());
    assert!(bootstrap.owner().is_none());
    assert!(bootstrap.failure_source().is_some());
    let original = bootstrap.scope.as_ref().ok_or("Rejected scope lost")?;
    assert!(original.snapshot().cancellation_requested);
    assert_eq!(original.snapshot().reserved_bytes, 0);
    drop(bootstrap);
    drop(runtime);
    files.finish()?;
    Ok(())
}

#[test]
fn close_before_start_keeps_given_actual_scope_without_starting_work() -> TestResult {
    let files = Files::new()?;
    let runtime = Runtime::new()?;
    let scope = runtime.scope(1)?;
    let mut bootstrap = files.bootstrap()?;
    bootstrap.request_close();
    assert_eq!(
        bootstrap.start(&runtime.client, scope, &JobSequence::new()),
        Err(EntryBootstrapIssue::Closed)
    );
    assert!(bootstrap.scope.is_some());
    assert!(bootstrap.preparation.is_none());
    assert!(bootstrap.owner().is_none());
    drop(bootstrap);
    drop(runtime);
    files.finish()?;
    Ok(())
}

#[test]
fn repeated_start_cannot_replace_original_job_scope_or_budget() -> TestResult {
    let files = Files::new()?;
    let runtime = Runtime::new()?;
    let mut bootstrap = files.bootstrap()?;
    let jobs = JobSequence::new();
    bootstrap.start(&runtime.client, runtime.scope(1)?, &jobs)?;
    let original = bootstrap.preparation.as_ref().ok_or("Original job absent")?.spec();
    assert_eq!(
        bootstrap.start(&runtime.client, runtime.scope(2)?, &jobs),
        Err(EntryBootstrapIssue::Started)
    );
    assert_eq!(bootstrap.scope.as_ref().ok_or("Original scope absent")?.id(), ScopeId::new(1)?);
    assert_eq!(bootstrap.preparation.as_ref().ok_or("Original job absent")?.spec(), original);
    assert_eq!(runtime.failure(&mut bootstrap, &jobs)?, EntryBootstrapIssue::Preparation);
    drop(bootstrap);
    drop(runtime);
    files.finish()?;
    Ok(())
}

#[test]
fn cleanup_cutoff_cannot_renew_original_transaction_even_before_namespace_creation() -> TestResult {
    let files = Files::new()?;
    let runtime = Runtime::new()?;
    let cleanup =
        runtime.client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut bootstrap = files.bootstrap()?;
    let jobs = JobSequence::new();
    assert!(bootstrap.retire(&cleanup, &jobs, Moment::from_nanos(100))?);
    assert_eq!(
        bootstrap.retire(&cleanup, &jobs, Moment::from_nanos(101)),
        Err(EntryBootstrapIssue::Cleanup)
    );
    assert!(bootstrap.retire(&cleanup, &jobs, Moment::from_nanos(99))?);
    assert_eq!(bootstrap.retirement_until, Some(Moment::from_nanos(99)));
    drop(bootstrap);
    drop(cleanup);
    drop(runtime);
    files.finish()?;
    Ok(())
}
