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
fn queued_native_failure_preserves_original_and_separate_cleanup_causes() -> TestResult {
    let files = Files::new()?;
    let mut bootstrap = files.bootstrap()?;
    let primary = native_error(&bootstrap)?;
    let cleanup = native_error(&bootstrap)?;
    for cause in [&primary, &cleanup] {
        assert!(matches!(
            bootstrap.observe(Some(EntryObservation::Failed(cause.clone()))),
            Err(EntryBootstrapIssue::NativeOwner)
        ));
    }
    assert!(bootstrap.closing);
    let leaf = |cause: &dyn Error| {
        cause.source().and_then(Error::source).map(|source| std::ptr::from_ref(source).cast::<()>())
    };
    assert_eq!(
        leaf(bootstrap.failure_source().ok_or("Primary native cause lost")?),
        leaf(&primary)
    );
    assert_eq!(
        leaf(bootstrap.cleanup_failure().ok_or("Cleanup native cause lost")?),
        leaf(&cleanup)
    );
    assert!(leaf(&primary).is_some());
    assert_ne!(leaf(&primary), leaf(&cleanup));
    drop(bootstrap);
    files.finish()?;
    Ok(())
}

/// Obtain a genuine owner rejection before any filesystem or scheduling effects.
fn native_error(bootstrap: &EntryBootstrap) -> Result<EntryError, Box<dyn Error>> {
    use sugarsubstitute_contracts::{
        configuration::{
            ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
            Theme,
        },
        coordinator_entry::EntryLaunchRequest,
        execution::{CompletionNotification, JobId},
    };
    let path = bootstrap.installation.clone().ok_or("Original location absent")?;
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("entry-failure".into())?,
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
    let input = CoordinatorEntryInput::new(
        path.clone(),
        path,
        sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Account,
        request,
    )?;
    let mut owner = CoordinatorEntryOwner::new(input, bootstrap.original);
    owner
        .consume(&CompletionBatch {
            notifications: vec![
                CompletionNotification {
                    scope_id: ScopeId::new(1)?,
                    job_id: JobId::new(1)?,
                    kind: TerminalKind::Failure,
                    replacement: None,
                };
                33
            ],
            remaining: 0,
        })
        .err()
        .ok_or_else(|| "Over-bound native observation accepted".into())
}

#[test]
fn native_degradation_closes_admission_without_waiting_for_setup_expiry() -> TestResult {
    let files = Files::new()?;
    let mut bootstrap = files.bootstrap()?;
    assert!(matches!(
        bootstrap.observe(Some(EntryObservation::Degraded(EntryOwnerDegradation::RecordUnknown))),
        Err(EntryBootstrapIssue::NativeOwner)
    ));
    assert!(bootstrap.closing);
    assert_eq!(
        bootstrap.failure_source().ok_or("Native cause lost")?.to_string(),
        "Production entry bootstrap failed: Degraded(RecordUnknown)"
    );
    drop(bootstrap);
    files.finish()?;
    Ok(())
}

#[test]
fn native_stop_is_preserved_and_cleanup_stop_does_not_replace_it() -> TestResult {
    let files = Files::new()?;
    let mut bootstrap = files.bootstrap()?;
    assert!(matches!(
        bootstrap.observe(Some(EntryObservation::Stopped(TerminalKind::TimedOut))),
        Err(EntryBootstrapIssue::NativeOwner)
    ));
    assert!(bootstrap.closing);
    assert!(matches!(
        bootstrap.observe(Some(EntryObservation::Stopped(TerminalKind::Cancelled)))?,
        Some(EntryObservation::Stopped(TerminalKind::Cancelled))
    ));
    assert_eq!(
        bootstrap.failure_source().ok_or("Native stop lost")?.to_string(),
        "Production entry bootstrap failed: Stopped(TimedOut)"
    );
    drop(bootstrap);
    files.finish()?;
    Ok(())
}

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
    assert!(bootstrap.failed_preparation.is_none());
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
    let original =
        bootstrap.migration.failure_completion().ok_or("Missing original preparation")?;
    assert_eq!(original.scope_id, bootstrap.scope.as_ref().ok_or("Scope lost")?.id());
    assert_eq!(original.spec.deadline, Some(bootstrap.original.until()));
    assert_eq!(original.outcome.kind(), TerminalKind::Failure);
    assert_eq!(
        bootstrap.scope.as_ref().ok_or("Scope lost")?.snapshot().reserved_bytes,
        sugarsubstitute_supervision::setup_storage::LAUNCH_DATA_PREPARATION_BYTES
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
    let original = bootstrap.migration.job_spec().ok_or("Original job absent")?;
    assert_eq!(
        bootstrap.start(&runtime.client, runtime.scope(2)?, &jobs),
        Err(EntryBootstrapIssue::Started)
    );
    assert_eq!(bootstrap.scope.as_ref().ok_or("Original scope absent")?.id(), ScopeId::new(1)?);
    assert_eq!(bootstrap.migration.job_spec().ok_or("Original job absent")?, original);
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
