// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real cancellation and typed worker/cleanup deliveries govern passive restoration and failure.
use super::*;
#[test]
fn export_unsubmitted_recovery_rejects_actual_admitted_delivery() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut controller = client
        .submit_blocking(&scope, spec(1, 2_097_152)?, |context| ExportService::new(&context))?;
    let completion = complete(&client, &wake, &mut controller)?;
    let JobOutcome::Success(mut service) = completion.outcome else {
        return Err("Controller admission failed".into());
    };
    drop(completion.memory);
    assert!(service.take_unsubmitted_input()?.is_none());
    assert!(!service.checked_terminal());
    service.delivery[0].job =
        Some(client.submit_blocking(&scope, spec(2, 131_072)?, |_| Ok::<_, ExportIssue>(true))?);
    assert_eq!(service.take_unsubmitted_input().err(), Some(ExportIssue::Started));
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        service.capture(&client.drain_completions(NonZeroUsize::MIN)?)?;
        if service.delivery[0].completion.is_some() {
            break;
        }
        wait(&wake, before)?;
    }
    assert_eq!(service.take_unsubmitted_input().err(), Some(ExportIssue::Started));
    assert!(!service.checked_terminal());
    assert!(service.receipt().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 2_097_152 + 131_072);
    drop(service);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn export_unsubmitted_recovery_observes_both_original_held_cancellations() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut controller = client
        .submit_blocking(&scope, spec(1, 2_097_152)?, |context| ExportService::new(&context))?;
    let done = complete(&client, &wake, &mut controller)?;
    let JobOutcome::Success(mut service) = done.outcome else {
        return Err("Controller source unavailable".into());
    };
    drop(done.memory);
    let mut scopes = Vec::new();
    for id in 2..=5 {
        scopes
            .push(client.create_scope(ScopeSpec { id: ScopeId::new(id)?, deadline: None }, None)?);
    }
    let mut source_cleanup = spec(2, 32_768)?;
    source_cleanup.class = JobClass::InfrastructureLifecycle;
    source_cleanup.deadline = None;
    let mut receiving_cleanup = source_cleanup;
    receiving_cleanup.id = JobId::new(3)?;
    service.reserve_source_cleanup(
        &client,
        &scopes[0],
        source_cleanup,
        &scopes[2],
        spec(4, 8_159_232)?,
    )?;
    service.reserve_receiving_cleanup(
        &client,
        &scopes[1],
        receiving_cleanup,
        &scopes[3],
        spec(5, 131_072)?,
    )?;
    assert!(service.take_unsubmitted_input()?.is_none());
    assert_eq!(service.bind(&mut None), Err(ExportIssue::Started));
    assert_eq!(
        service.reserve_source_cleanup(
            &client,
            &scopes[0],
            source_cleanup,
            &scopes[2],
            spec(4, 8_159_232)?
        ),
        Err(ExportIssue::Identity)
    );
    assert_eq!(service.release_unsubmitted(), Err(ExportIssue::Busy));
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        service
            .capture(&client.drain_completions(NonZeroUsize::new(8).ok_or("Completion bound")?)?)?;
        if service.unsubmitted_observed() {
            break;
        }
        wait(&wake, before)?;
    }
    assert!(
        service.delivery[..2].iter().all(|lane| lane.job.is_none() && lane.completion.is_none())
    );
    for lane in &service.delivery[2..] {
        assert!(matches!(
            lane.completion.as_ref().map(|done| &done.outcome),
            Some(JobOutcome::Cancelled)
        ));
    }
    assert!(!service.checked_terminal());
    assert!(!service.expected_receiving_failure());
    assert!(service.receipt().is_none());
    assert_eq!(scopes[0].snapshot().reserved_bytes, 32_768);
    assert_eq!(scopes[1].snapshot().reserved_bytes, 32_768);
    assert_eq!(scopes[2].snapshot().reserved_bytes, 0);
    assert_eq!(scopes[3].snapshot().reserved_bytes, 0);
    assert!(service.take_unsubmitted_input()?.is_none());
    drop(controller);
    drop(owner);
    service.release_unsubmitted()?;
    assert_eq!(scopes[0].snapshot().reserved_bytes, 0);
    assert_eq!(scopes[1].snapshot().reserved_bytes, 0);
    assert_eq!(scope.snapshot().reserved_bytes, 2_097_152);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn export_worker_failure_keeps_actual_cause_and_cannot_complete_as_save_failure() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut controller = client
        .submit_blocking(&scope, spec(1, 2_097_152)?, |context| ExportService::new(&context))?;
    let done = complete(&client, &wake, &mut controller)?;
    let JobOutcome::Success(mut service) = done.outcome else {
        return Err("Controller source unavailable".into());
    };
    drop(done.memory);
    service.delivery[0].job =
        Some(client.submit_blocking(&scope, spec(2, 131_072)?, |_| {
            Err::<bool, _>(ExportIssue::Identity)
        })?);
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        service.capture(&client.drain_completions(NonZeroUsize::MIN)?)?;
        if service.delivery[0].completion.is_some() {
            break;
        }
        wait(&wake, before)?;
    }
    assert_eq!(
        service.inspect_execution_failure(|done| matches!(
            done.outcome,
            JobOutcome::Failure(ExportIssue::Identity)
        )),
        Some(true)
    );
    assert_eq!(
        service.inspect_failure(|cause, _| cause
            .and_then(std::error::Error::source)
            .map(std::string::ToString::to_string)),
        Some(Some(ExportIssue::Identity.to_string()))
    );
    assert!(!service.expected_receiving_failure());
    assert!(!service.checked_terminal());
    assert!(service.take_terminal().is_none());
    drop(controller);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 2_097_152 + 131_072);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn export_typed_cleanup_rejection_stays_strict_and_retains_actual_delivery() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut controller = client
        .submit_blocking(&scope, spec(1, 2_097_152)?, |context| ExportService::new(&context))?;
    let done = complete(&client, &wake, &mut controller)?;
    let JobOutcome::Success(mut service) = done.outcome else {
        return Err("Controller source unavailable".into());
    };
    drop(done.memory);
    // This actual job supplies only a delivery failure; it never grants a native cleanup proof.
    service.delivery[2].job =
        Some(client.submit_blocking(&scope, spec(2, 32_768)?, |_| Ok::<_, ExportIssue>(false))?);
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        service.capture(&client.drain_completions(NonZeroUsize::MIN)?)?;
        if service.delivery[2].completion.is_some() {
            break;
        }
        wait(&wake, before)?;
    }
    assert!(service.cleanup_failed());
    assert!(service.inspect_execution_failure(|_| ()).is_none());
    assert!(!service.checked_terminal());
    assert!(!service.expected_receiving_failure());
    assert!(service.take_terminal().is_none());
    drop(controller);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 2_097_152 + 32_768);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
