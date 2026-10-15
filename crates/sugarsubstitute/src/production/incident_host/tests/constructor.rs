// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit the actual boxed constructor using a genuine consumed entry namespace.
//! Only the entry fixture's external native boundary is inert; no native qualification is claimed.
use super::*;
use sugarsubstitute_contracts::{configuration::*, coordinator_entry::*};
use sugarsubstitute_supervision::coordinator_entry::{
    self, CoordinatorEntryOwner, EntryObservation,
};
#[path = "diagnostics.rs"]
mod diagnostics;
#[path = "policy_loan.rs"]
mod policy_loan;
#[path = "readiness.rs"]
mod readiness;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn entry_owner() -> TestResult<CoordinatorEntryOwner> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| value.checked_add(1))
        .map_err(|_| "Inert path identity exhausted")?;
    let path = std::env::temp_dir()
        .join(format!("private-constructor-fixture-{}-{id}", std::process::id()));
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("Profile_1".to_owned())?,
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
    Ok(coordinator_entry::namespace_fixture::namespace_owner(
        coordinator_entry::CoordinatorEntryInput::new(
            path.clone(),
            path,
            coordinator_entry::DiagnosticStorageLocation::Account,
            request,
        )?,
        EntrySetupBudget::new(Moment::from_nanos(0))?,
    ))
}

/// Wait for a causal completion notification, with a wall-clock fail-safe for deadlocks.
fn delivery<T>(
    client: &ExecutionClient,
    wake: &Wake,
    mut consume: impl FnMut(&CompletionBatch) -> TestResult<Option<T>>,
) -> TestResult<T> {
    loop {
        let before = *wake.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(value) = consume(&batch)? {
            return Ok(value);
        }
        if batch.remaining > 0 {
            continue;
        }
        let (guard, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |value| *value == before,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(guard);
        if timeout.timed_out() {
            return Err("Actual entry/constructor completion was not delivered".into());
        }
    }
}

fn namespace(
    entry: &mut CoordinatorEntryOwner,
    client: &ExecutionClient,
    scope: &Scope,
    wake: &Wake,
    jobs: &JobSequence,
) -> TestResult<VerifiedEntryNamespace> {
    let mut specification = spec(jobs.allocate()?);
    specification.reservation_bytes = coordinator_entry::ENTRY_PREPARATION_BYTES;
    specification.deadline = Some(entry.original_setup_budget().until());
    entry.submit_discovery(client, scope, specification)?;
    assert!(matches!(
        delivery(client, wake, |batch| Ok(entry.consume(batch)?))?,
        EntryObservation::NamespacePrepared(_)
    ));
    let namespace = entry.take_namespace().ok_or("No consumed namespace")?;
    assert!(entry.take_namespace().is_none());
    Ok(namespace)
}

fn captured(host: &mut IncidentHost, client: &ExecutionClient, wake: &Wake) -> TestResult {
    delivery(client, wake, |batch| {
        let result = host.capture(batch);
        if host.completion.is_some() {
            return Ok(Some(result.map(|_| ())));
        }
        result?;
        Ok(None)
    })??;
    Ok(())
}

fn retire_entry(
    entry: &mut CoordinatorEntryOwner,
    client: &ExecutionClient,
    scope: &Scope,
    wake: &Wake,
    jobs: &JobSequence,
) -> TestResult {
    entry.close_admission();
    let mut specification = spec(jobs.allocate()?);
    specification.reservation_bytes = coordinator_entry::ENTRY_PREPARATION_BYTES;
    entry.submit_close(client, scope, specification)?;
    assert!(matches!(
        delivery(client, wake, |batch| Ok(entry.consume(batch)?))?,
        EntryObservation::Retired
    ));
    Ok(())
}

#[test]
fn submit_prepare_consumes_genuine_namespace_and_retains_original_charge_through_join() -> TestResult
{
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut entry = entry_owner()?;
    let namespace = namespace(&mut entry, &client, &scope, &wake, &jobs)?;
    let until = namespace.original_preparation().until();
    let mut input = Some(PreparationInput::new(namespace, key()?));
    let mut host = IncidentHost::new();
    host.submit_prepare(&client, &scope, &jobs, until, &mut input)?;
    assert!(input.is_none());
    captured(&mut host, &client, &wake)?;
    assert!(host.prepared());
    assert_eq!(host.preparation_outcome(), Some(TerminalKind::Success));
    assert_eq!(host.namespace()?.execution_owner(), scope.owner_id());
    retire_entry(&mut entry, &client, &scope, &wake, &jobs)?;
    assert!(host.namespace()?.validate_live().is_err());
    drop(entry);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(host);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn submit_prepare_logs_closed_namespace_once_and_retains_original_error_and_input() -> TestResult {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut entry = entry_owner()?;
    let namespace = namespace(&mut entry, &client, &scope, &wake, &jobs)?;
    let until = namespace.original_preparation().until();
    entry.close_admission();
    let mut input = Some(PreparationInput::new(namespace, key()?));
    let mut host = IncidentHost::new();
    host.submit_prepare(&client, &scope, &jobs, until, &mut input)?;
    assert!(input.is_none());
    let output = diagnostics::Output::default();
    let _subscriber = output.observe();
    let error = captured(&mut host, &client, &wake).err().ok_or("Expected actual failure")?;
    assert_eq!(error.downcast_ref::<HostIssue>(), Some(&HostIssue::Preparation));
    assert_eq!(host.preparation_outcome(), Some(TerminalKind::Failure));
    assert!(!host.prepared());
    let original = host.failure().and_then(Error::source).ok_or("Original entry error lost")?;
    assert!(original.is::<EntryError>());
    let original = std::ptr::from_ref(original);
    assert!(!host.capture(&client.drain_completions(NonZeroUsize::MIN)?)?);
    assert!(std::ptr::eq(
        original,
        std::ptr::from_ref(host.failure().and_then(Error::source).ok_or("Error replaced")?)
    ));
    let text = output.text()?;
    assert_eq!(text.matches("incident_constructor_terminal").count(), 1);
    assert!(text.contains("terminal=Failure"));
    assert!(text.contains("preparation_cause=\"namespace\""));
    assert!(text.contains("identity_stage=None"));
    assert!(text.contains("queue_time_ns=0"));
    assert!(text.contains("execution_time_ns=0"));
    assert!(text.contains(&format!("deadline_ns={}", until.as_nanos())));
    assert!(!text.contains("private-constructor-fixture"));
    retire_entry(&mut entry, &client, &scope, &wake, &jobs)?;
    drop(entry);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(host);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn constructor_memory_diagnostic_reports_original_requested_and_available_bytes() -> TestResult {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut entry = entry_owner()?;
    let namespace = namespace(&mut entry, &client, &scope, &wake, &jobs)?;
    let mut input = Some(PreparationInput::new(namespace, key()?));
    let mut host = IncidentHost::new();
    // Exhaust actual original capacity to exercise failure observation, not constructor success.
    host.constructor = Some(client.submit_blocking_owned_box(
        &scope,
        spec(jobs.allocate()?),
        &mut input,
        |input, context| {
            let cause = match context.memory().claim(POLICY_BYTES) {
                Ok(_held) => {
                    match super::super::admission::allocate_policy(input.input().owner, &context) {
                        Err(cause) => cause,
                        Ok(_) => {
                            return Err(PreparationFailure {
                                cause: PreparationCause::Capacity,
                                _input: input,
                            });
                        }
                    }
                }
                Err(cause) => PreparationCause::Memory(cause),
            };
            Err(PreparationFailure { cause, _input: input })
        },
    )?);
    let output = diagnostics::Output::default();
    let _subscriber = output.observe();
    assert!(captured(&mut host, &client, &wake).is_err());
    let cause = host
        .failure()
        .and_then(Error::source)
        .and_then(|cause| cause.downcast_ref::<MemoryClaimError>())
        .ok_or("Original memory cause lost")?;
    assert_eq!(cause.requested_bytes, POLICY_BYTES);
    assert_eq!(
        cause.available_bytes,
        CONTROLLER_BYTES - size_of::<PreparationInput>() as u64 - POLICY_BYTES
    );
    let text = output.text()?;
    assert!(text.contains("terminal=Failure"));
    assert!(text.contains("preparation_cause=\"memory\""));
    assert!(text.contains(&format!("requested_bytes={}", cause.requested_bytes)));
    assert!(text.contains(&format!("available_bytes={}", cause.available_bytes)));
    assert!(!text.contains("private-constructor-fixture"));
    retire_entry(&mut entry, &client, &scope, &wake, &jobs)?;
    drop(entry);
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(host);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn constructor_panic_diagnostic_keeps_payload_private_and_original_completion_owned() -> TestResult
{
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut host = IncidentHost::new();
    // Exercise terminal observation only: this job grants no namespace or prepared controller.
    host.constructor = Some(client.submit_blocking(&scope, spec(JobId::new(1)?), |_context| {
        std::panic::resume_unwind(Box::new("private-panic-token-and-path".to_owned()));
    })?);
    let output = diagnostics::Output::default();
    let _subscriber = output.observe();
    assert!(captured(&mut host, &client, &wake).is_err());
    assert_eq!(host.preparation_outcome(), Some(TerminalKind::Panicked));
    assert!(!host.prepared());
    let original = host.panic_cause().ok_or("Original panic lost")?;
    assert!(original.as_str().contains("private-panic-token-and-path"));
    let text = output.text()?;
    assert!(text.contains("terminal=Panicked"));
    assert!(text.contains("preparation_cause=\"none\""));
    assert!(text.contains("identity_stage=None"));
    assert!(text.contains("queue_time_ns=0"));
    assert!(text.contains("execution_time_ns=0"));
    assert!(text.contains("deadline_ns=30000000000"));
    assert!(!text.contains("private-panic-token-and-path"));
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(host);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
