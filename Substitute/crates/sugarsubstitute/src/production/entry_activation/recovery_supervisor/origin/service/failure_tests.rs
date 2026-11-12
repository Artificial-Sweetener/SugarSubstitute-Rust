// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose real Core funding and actual repository custody with borrowed service failure reporting.
use super::super::test_support::*;
use super::*;
use sugarsubstitute_supervision::diagnostic_storage::fixture::{
    DiagnosticFixtureFailure, DiagnosticStorageFixture,
};

#[test]
fn origin_service_keeps_actual_panic_first_and_cleanup_source_funded_through_last_drop()
-> Result<(), TestError> {
    let (owner, client, wake, scope) = owner()?;
    let jobs = JobSequence::new();
    let source = source::tests::source(&client, &scope, &wake, &jobs)?;
    let fixture = Arc::new(DiagnosticStorageFixture::new()?);
    let worker_source = source.alias()?;
    let external = fixture.clone();
    let mut worker = client.submit_blocking(
        &scope,
        spec(jobs.allocate()?, DIAGNOSTIC_STORAGE_BYTES),
        move |context| -> Result<(), TestError> {
            let _loan = source::tests::prepared_loan(&worker_source, &context, &external)?;
            std::panic::resume_unwind(Box::new(991_u64))
        },
    )?;
    let done = observe(&client, &wake, &mut worker)?;
    let JobCompletion {
        scope_id,
        spec: original_spec,
        queue_time,
        execution_time,
        outcome,
        memory,
        native_handles,
    } = done;
    let panic = match outcome {
        JobOutcome::Panicked(cause) => cause,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("actual producer did not preserve its panic".into()),
    };
    let original_text = panic.as_str().as_ptr();
    let mut service = OriginPublication::from_source(source.alias()?);
    service.completion = Some(RetainedOutcome {
        original: JobCompletion {
            scope_id,
            spec: original_spec,
            queue_time,
            execution_time,
            outcome: JobOutcome::Panicked(panic),
            memory,
            native_handles,
        },
    });
    let actual = service
        .failure()
        .and_then(|cause| cause.downcast_ref::<RetainedOutcome<()>>())
        .ok_or("actual panic completion missing from service cause")?;
    match &actual.original.outcome {
        JobOutcome::Panicked(cause) => assert_eq!(cause.as_str().as_ptr(), original_text),
        _ => return Err("original panic replaced".into()),
    }
    fixture.set_failure(DiagnosticFixtureFailure::Close, true);
    assert!(!service.close(&client, &scope, &jobs)?);
    capture_until(&client, &wake, |batch| {
        service.capture(batch)?;
        Ok(service.cleanup.is_none())
    })?;
    assert!(service.cleanup_failure().and_then(Error::source).and_then(Error::source).is_some());
    assert!(service.failure().is_some_and(|cause| cause.is::<RetainedOutcome<()>>()));
    assert!(fixture.snapshot().live_outputs() > 0);
    // Reconcile only the same physical source under the SAME already-issued cleanup cutoff.
    let original_until = service.cleanup_budget.ok_or("original cleanup cutoff lost")?.settle_until;
    fixture.set_failure(DiagnosticFixtureFailure::Close, false);
    let cleanup_source = source.alias()?;
    let mut cleanup_spec = spec(jobs.allocate()?, 32 * 1024);
    cleanup_spec.deadline = Some(original_until);
    let mut checked = client.submit_blocking(&scope, cleanup_spec, move |context| {
        cleanup_source.loan(false)?.retire(&context)
    })?;
    let checked_done = observe(&client, &wake, &mut checked)?;
    assert!(matches!(checked_done.outcome, JobOutcome::Success(true)));
    service.retain_provider_failures();
    assert!(service.failure().is_some_and(|cause| cause.is::<RetainedOutcome<()>>()));
    assert!(source.retired());
    assert_eq!(fixture.snapshot().live_outputs(), 0);
    let escaped =
        service.provider_cleanup_failure.take().ok_or("original cleanup cause missing")?;
    assert_eq!(escaped.category(), OriginFailureCategory::RepositoryCleanup);
    drop(checked_done);
    drop(checked);
    drop(worker);
    drop(service);
    drop(source);
    drop(fixture);
    drop(owner);
    assert!(scope.snapshot().reserved_bytes >= 4 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES);
    assert!(escaped.source().and_then(Error::source).is_some());
    drop(escaped);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn origin_service_borrows_actual_admission_before_cleanup_without_replacing_source_funding()
-> Result<(), TestError> {
    let (owner, client, wake, scope) = owner()?;
    let jobs = JobSequence::new();
    let source = source::tests::source(&client, &scope, &wake, &jobs)?;
    let mut rejected_spec = spec(jobs.allocate()?, DIAGNOSTIC_STORAGE_BYTES);
    rejected_spec.deadline = Some(Moment::from_nanos(1));
    let admission =
        match client.submit_blocking(&scope, rejected_spec, |_| Ok::<_, OriginIssue>(())) {
            Err(cause) => cause,
            Ok(_) => return Err("expired actual Core admission unexpectedly succeeded".into()),
        };
    let mut service = OriginPublication::from_source(source);
    service.admission = Some(admission);
    let retained = service.admission.as_ref().ok_or("original admission lost")?;
    let exposed = service
        .failure()
        .and_then(|cause| cause.downcast_ref::<AdmissionError>())
        .ok_or("actual admission source unreachable")?;
    assert!(std::ptr::eq(retained, exposed));
    let cleanup_cause = match ShutdownBudget::new(Moment::from_nanos(u64::MAX)) {
        Err(cause) => cause,
        Ok(_) => return Err("overflowing cleanup deadline unexpectedly succeeded".into()),
    };
    service.cleanup_time = Some(cleanup_cause);
    assert!(service.cleanup_failure().is_some_and(|cause| cause.is::<LifecycleRejection>()));
    assert!(service.failure().is_some_and(|cause| cause.is::<AdmissionError>()));
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 4 * 1024 * 1024);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
