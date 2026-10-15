// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Genuine capacity rejection cancels actual partial holds; stale identity never becomes a retry.
use super::*;
use crate::job_sequence::JobSequence;
use sugarsubstitute_execution::ReportExportPairRequest;

/// Await one exact actual terminal notice without acknowledging its retained job credit.
fn observed(client: &ExecutionClient, wake: &Wake, id: JobId) -> TestResult {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if batch.notifications.iter().any(|notice| notice.job_id == id) {
            return Ok(());
        }
        wait(wake, before)?;
    }
}
/// The actual cancellation jobs and their correlated notices, never a copied native flag.
fn cancelled(client: &ExecutionClient, wake: &Wake, service: &mut ExportService) -> TestResult {
    assert!(service.take_unsubmitted_input()?.is_none());
    while !service.unsubmitted_observed() {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        service.capture(&client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?)?;
        if !service.unsubmitted_observed() {
            wait(wake, before)?;
        }
    }
    Ok(())
}
/// Four distinct independent real scopes preserve the central cleanup isolation rules.
fn scopes(client: &ExecutionClient) -> Result<[Scope; 4], TestError> {
    Ok([
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?,
        client.create_scope(ScopeSpec { id: ScopeId::new(4)?, deadline: None }, None)?,
        client.create_scope(ScopeSpec { id: ScopeId::new(5)?, deadline: None }, None)?,
    ])
}
/// Preserve the same100ns original Main cutoff through all attempts and cleanup associations.
fn main_spec(id: JobId, bytes: u64) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: bytes,
        replacement: None,
    }
}
fn cleanup_spec(id: JobId) -> JobSpec {
    JobSpec {
        id,
        class: JobClass::InfrastructureLifecycle,
        deadline: None,
        reservation_bytes: 32_768,
        ..main_spec(id, 32_768)
    }
}

#[test]
fn stale_export_cleanup_id_is_permanent_and_retains_actual_order_cause_without_native_success()
-> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let sequence = JobSequence::new();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut controller = client.submit_blocking(
        &scope,
        spec(sequence.allocate()?.get(), 2_097_152)?,
        |context| ExportService::new(&context),
    )?;
    let done = complete(&client, &wake, &mut controller)?;
    let JobOutcome::Success(mut service) = done.outcome else {
        return Err("Actual source missing".into());
    };
    drop(done.memory);
    let scopes = scopes(&client)?;
    let stale = cleanup_spec(sequence.allocate()?);
    let source = main_spec(sequence.allocate()?, 8_159_232);
    let later = sequence.allocate()?;
    let mut view =
        client.submit_blocking(&scope, spec(later.get(), 4096)?, |_| Ok::<_, ExportIssue>(true))?;
    drop(complete(&client, &wake, &mut view)?);
    assert_eq!(
        service.reserve_source_cleanup(&client, &scopes[0], stale, &scopes[2], source),
        Err(ExportIssue::Rejected)
    );
    assert!(
        matches!(service.admission_failure(), Some(AdmissionError::JobIdentityOrder { latest }) if *latest == later)
    );
    assert_eq!(
        service.inspect_failure(|cause, _| cause
            .and_then(|cause| cause.downcast_ref::<AdmissionError>())
            .is_some()),
        Some(true)
    );
    assert!(!service.checked_terminal());
    assert!(service.receipt().is_none());
    assert!(service.source_held.is_none());
    assert_eq!(scopes[0].snapshot().reserved_bytes, 0);
    drop(controller);
    drop(owner);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn partial_export_overload_observes_original_cancellation_before_fresh_user_request_can_admit_pair()
-> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let sequence = JobSequence::new();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut controller = client.submit_blocking(
        &scope,
        spec(sequence.allocate()?.get(), 2_097_152)?,
        |context| ExportService::new(&context),
    )?;
    let done = complete(&client, &wake, &mut controller)?;
    let JobOutcome::Success(mut service) = done.outcome else {
        return Err("Actual source missing".into());
    };
    drop(done.memory);
    let scopes = scopes(&client)?;
    let mut retained = Vec::new();
    retained.try_reserve_exact(47)?;
    for _ in 0..47 {
        let id = sequence.allocate()?;
        let mut specification = spec(id.get(), 4096)?;
        specification.class = JobClass::InfrastructureLifecycle;
        let job = client.submit_blocking(&scope, specification, |_| Ok::<_, ExportIssue>(true))?;
        observed(&client, &wake, id)?;
        retained.push(job);
    }
    let source_cleanup = cleanup_spec(sequence.allocate()?);
    let receiving_cleanup = cleanup_spec(sequence.allocate()?);
    let source = main_spec(sequence.allocate()?, 8_159_232);
    let receiving = main_spec(sequence.allocate()?, 131_072);
    service.reserve_source_cleanup(&client, &scopes[0], source_cleanup, &scopes[2], source)?;
    assert_eq!(
        service.reserve_receiving_cleanup(
            &client,
            &scopes[1],
            receiving_cleanup,
            &scopes[3],
            receiving
        ),
        Err(ExportIssue::Admission)
    );
    assert!(matches!(service.admission_failure(), Some(AdmissionError::CompletionSaturated)));
    assert_eq!(scopes[0].snapshot().reserved_bytes, 32_768);
    assert!(!service.checked_terminal());
    assert!(service.receipt().is_none());
    // Release exactly one real terminal credit, then interleave another owner before rollback.
    let mut released = retained.remove(0);
    drop(released.try_take_completion().ok_or("Observed pressure completion absent")?);
    let unrelated_id = sequence.allocate()?;
    let mut unrelated_spec = spec(unrelated_id.get(), 4096)?;
    unrelated_spec.class = JobClass::InfrastructureLifecycle;
    let mut unrelated =
        client.submit_blocking(&scope, unrelated_spec, |_| Ok::<_, ExportIssue>(true))?;
    observed(&client, &wake, unrelated_id)?;
    cancelled(&client, &wake, &mut service)?;
    let old =
        service.delivery[3].completion.as_ref().ok_or("Actual unpaired cancellation absent")?;
    assert_eq!(old.spec, source_cleanup);
    assert!(matches!(old.outcome, JobOutcome::Cancelled));
    assert!(
        service.delivery[..2]
            .iter()
            .all(|delivery| delivery.job.is_none() && delivery.completion.is_none())
    );
    assert!(!service.checked_terminal());
    assert!(!service.expected_receiving_failure());
    assert!(service.take_terminal().is_none());
    service.release_unsubmitted()?;
    assert_eq!(scopes[0].snapshot().reserved_bytes, 0);
    // A new authored request may start after the failed request has actually retired.
    // No old permit/spec is edited or reused; central admission decides the new attempt.
    for _ in 0..4 {
        let mut job = retained.remove(0);
        drop(job.try_take_completion().ok_or("Actual pressure terminal missing")?);
    }
    let source_cleanup = cleanup_spec(sequence.allocate()?);
    let receiving_cleanup = cleanup_spec(sequence.allocate()?);
    let source = main_spec(sequence.allocate()?, 8_159_232);
    let receiving = main_spec(sequence.allocate()?, 131_072);
    assert!(source_cleanup.id > unrelated_id);
    service.reserve_source_cleanup(&client, &scopes[0], source_cleanup, &scopes[2], source)?;
    service.reserve_receiving_cleanup(
        &client,
        &scopes[1],
        receiving_cleanup,
        &scopes[3],
        receiving,
    )?;
    let (source_job, receiving_job) = client
        .submit_report_export_pair(
            ReportExportPairRequest::new(
                &scopes[2],
                source,
                service.source_held.as_ref().ok_or("Source hold absent")?,
                &scopes[3],
                receiving,
                service.receiving_held.as_ref().ok_or("Receiving hold absent")?,
            ),
            |source, receiving| {
                assert_eq!(source.deadline(), Some(Moment::from_nanos(100)));
                assert_eq!(receiving.deadline(), source.deadline());
                Ok::<_, ExportIssue>(true)
            },
        )
        .map_err(|rejection| rejection.error)?;
    service.delivery[0].job = Some(source_job);
    service.delivery[1].job = Some(receiving_job);
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        service.capture(&client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?)?;
        service.advance_cleanup(&client)?;
        if service.checked_terminal() {
            break;
        }
        wait(&wake, before)?;
    }
    assert!(service.delivery.iter().all(delivery::Delivery::succeeded));
    assert!(service.receipt().is_none()); // This admission case produced no native archive receipt.
    drop(unrelated.try_take_completion().ok_or("Unrelated terminal missing")?);
    drop(retained);
    drop(controller);
    drop(owner);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
