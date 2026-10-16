// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real service delivery and cleanup with an external protected filesystem fixture.
use super::super::tests::{prepared, summary};
use super::*;
use crate::production::diagnostic_helper::tests::{TestClock, TestError, Wake, observe, spec};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_crash::incident::{StoredIncidentSummary, encode_summary};
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_supervision::diagnostic_storage::fixture::{
    DiagnosticFixtureFailure, DiagnosticStorageFixture,
};

fn delivered(
    service: &mut RecoveryService,
    client: &ExecutionClient,
    wake: &Wake,
    cleanup: bool,
) -> Result<(), TestError> {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        service.capture(&batch)?;
        if if cleanup { service.cleanup.completion.is_some() } else { service.delivered() } {
            return Ok(());
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == before,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && *count == before {
            return Err("Actual recovery delivery stalled".into());
        }
    }
}
#[test]
fn actual_service_requires_independent_checked_cleanup_before_snapshot_delivery_and_disposal()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut metadata = spec(jobs.allocate()?);
    metadata.reservation_bytes = RECOVERY_SOURCE_BYTES;
    let mut constructor =
        client.submit_blocking(&scope, metadata, |context| RecoveryService::new(&context))?;
    let done = observe(&client, &wake, &mut constructor)?;
    let mut service = match done.outcome {
        JobOutcome::Success(value) => value,
        _ => return Err("Actual service construction stopped".into()),
    };
    drop(done.memory);
    drop(constructor);
    let source = service.source.alias();
    // Inject only the external storage opening. No production namespace token, authenticated
    // request, native family, or historical operation is constructed by this fixture.
    service.work.job =
        Some(client.submit_blocking(&scope, spec(jobs.allocate()?), move |context| {
            let fixture = DiagnosticStorageFixture::new().map_err(|_| RecoveryIssue::Failed)?;
            let mut operation = prepared(&context, &fixture).map_err(|_| RecoveryIssue::Failed)?;
            let summary = summary().map_err(|_| RecoveryIssue::Failed)?;
            fixture
                .put(
                    DiagnosticObject::Summary(summary.seed().key()),
                    &encode_summary(
                        &StoredIncidentSummary::new(summary, None)
                            .map_err(|_| RecoveryIssue::Failed)?,
                    )
                    .map_err(|_| RecoveryIssue::Failed)?,
                )
                .map_err(|_| RecoveryIssue::Failed)?;
            operation.pending = Some(
                operation
                    .repository
                    .as_mut()
                    .ok_or(RecoveryIssue::Failed)?
                    .pending_incidents(
                        IncidentUtc::from_unix_seconds(110).map_err(|_| RecoveryIssue::Failed)?,
                        &context,
                    )
                    .map_err(|_| RecoveryIssue::Failed)?,
            );
            let mut loan = source.loan()?;
            *loan.operation()? = operation;
            Ok::<_, RecoveryIssue>(true)
        })?);
    service.installed = true;
    delivered(&mut service, &client, &wake, false)?;
    assert!(service.delivered());
    let actual = service.work.completion.as_ref().ok_or("Actual recovery Main missing")?;
    assert!(
        matches!(actual.outcome, JobOutcome::Success(true)),
        "Actual recovery Main outcome: {:?}",
        actual.outcome
    );
    assert!(!service.files_retired());
    assert!(service.inspect(|_| ()).is_err());
    assert!(service.release_adopted().is_err());
    service.submit_cleanup(&client, &scope, &jobs, Moment::from_nanos(100))?;
    delivered(&mut service, &client, &wake, true)?;
    assert!(service.files_retired());
    assert!(!service.retain_failure());
    let key = service.inspect(|pending| {
        Ok::<_, TestError>(
            pending.summaries()?.next().ok_or("Pending summary absent")?.summary().seed().key(),
        )
    })??;
    assert_eq!(key, summary()?.seed().key());
    drop(owner);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        RECOVERY_SOURCE_BYTES + DIAGNOSTIC_STORAGE_BYTES + 4096
    );
    service.release_adopted()?;
    assert!(!service.pending());
    assert_eq!(scope.snapshot().reserved_bytes, RECOVERY_SOURCE_BYTES);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Pause only the external fixture after the native loan has returned to its source.
/// The central worker and original deadline stay unchanged; no provider or source lock is held.
#[derive(Default)]
struct FailedCloseBoundary {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}
impl FailedCloseBoundary {
    fn failed_close(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = true;
        self.changed.notify_all();
        let (state, _) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1
    }
    fn observe_failed_close(&self) -> Result<(), TestError> {
        let (state, _) = self
            .changed
            .wait_timeout_while(
                self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |state| !state.0,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.0 { Ok(()) } else { Err("Actual failed-close boundary did not arrive".into()) }
    }
    fn release(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1 = true;
        self.changed.notify_all();
    }
}
struct ReleaseFailedClose(Arc<FailedCloseBoundary>);
impl Drop for ReleaseFailedClose {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[test]
fn completed_read_cause_and_uncertain_cleanup_remain_visible_with_original_escaped_charge()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut metadata = spec(jobs.allocate()?);
    metadata.reservation_bytes = RECOVERY_SOURCE_BYTES;
    let mut constructor =
        client.submit_blocking(&scope, metadata, |context| RecoveryService::new(&context))?;
    let done = observe(&client, &wake, &mut constructor)?;
    let metadata_reservation = done.memory;
    let mut service = match done.outcome {
        JobOutcome::Success(value) => value,
        _ => return Err("Actual recovery constructor stopped".into()),
    };
    drop(constructor);
    let fixture = Arc::new(DiagnosticStorageFixture::new()?);
    let source = service.source.alias();
    let disk = fixture.clone();
    let work_source = source.alias();
    service.work.job = Some(client.submit_blocking(
        &scope,
        spec(jobs.allocate()?),
        move |context| -> Result<bool, RecoveryIssue> {
            let mut operation = prepared(&context, &disk).map_err(|_| RecoveryIssue::Failed)?;
            let object = DiagnosticObject::Summary(
                summary().map_err(|_| RecoveryIssue::Failed)?.seed().key(),
            );
            disk.put(object, b"original corrupt pending summary")
                .map_err(|_| RecoveryIssue::Failed)?;
            let failure = operation
                .repository
                .as_mut()
                .ok_or(RecoveryIssue::Identity)?
                .pending_incidents(
                    IncidentUtc::from_unix_seconds(110).map_err(|_| RecoveryIssue::Failed)?,
                    &context,
                )
                .err()
                .ok_or(RecoveryIssue::Failed)?;
            operation.first = Some(failure);
            *work_source.loan()?.operation()? = operation;
            Ok(true)
        },
    )?);
    service.installed = true;
    delivered(&mut service, &client, &wake, false)?;
    let main_reservation = &service.work.completion.as_ref().ok_or("Actual Main absent")?.memory;
    let original = service.failure.as_ref().ok_or("Original read cause hidden before cleanup")?;
    assert!(original._original.same_reservation(main_reservation));
    assert!(!original._original.same_reservation(&metadata_reservation));
    assert!(original.retirement_failure().is_none());
    assert!(
        service
            .failure()
            .and_then(|cause| cause.source())
            .and_then(|cause| cause.source())
            .is_some_and(|cause| cause.downcast_ref::<IncidentStoreError>().is_some())
    );
    assert!(!service.files_retired());
    assert!(service.inspect(|_| ()).is_err());
    assert!(service.release_adopted().is_err());

    let boundary = Arc::new(FailedCloseBoundary::default());
    let release = ReleaseFailedClose(boundary.clone());
    let worker_boundary = boundary.clone();
    let worker_source = source.alias();
    let worker_disk = fixture.clone();
    fixture.set_failure(DiagnosticFixtureFailure::Close, true);
    let mut cleanup = spec(jobs.allocate()?);
    cleanup.reservation_bytes = 4096;
    cleanup.deadline = Some(Moment::from_nanos(100));
    service.cleanup.job = Some(client.submit_blocking(
        &scope,
        cleanup,
        move |context| -> Result<bool, RecoveryIssue> {
            {
                let mut loan = worker_source.loan()?;
                if loan.operation()?.retire(&context) {
                    return Err(RecoveryIssue::Failed);
                }
            }
            let released = worker_boundary.failed_close();
            // Finish only this fake filesystem under the SAME actual cleanup job/context.
            // Preserve the first failed return; no new job, claim or cleanup cutoff is minted.
            worker_disk.set_failure(DiagnosticFixtureFailure::Close, false);
            if !worker_source.loan()?.operation()?.retire(&context) || !released {
                return Err(RecoveryIssue::Failed);
            }
            Ok(false)
        },
    )?);
    boundary.observe_failed_close()?;
    assert_eq!(
        source.inspect(|operation| !operation.retired && operation.retirement.is_some()),
        Some(true)
    );
    assert!(service.retain_failure());
    assert!(service.failure.as_ref().and_then(RecoveryFailure::retirement_failure).is_some());
    assert!(
        service
            .failure()
            .and_then(|cause| cause.source())
            .and_then(|cause| cause.source())
            .is_some_and(|cause| cause.downcast_ref::<IncidentStoreError>().is_some())
    );
    assert_eq!(source.inspect(|operation| !operation.retired && operation.failed()), Some(true));
    assert!(!service.files_retired());
    assert!(source.release().is_err());
    assert_eq!(
        scope.snapshot().reserved_bytes,
        RECOVERY_SOURCE_BYTES + DIAGNOSTIC_STORAGE_BYTES + 4096
    );
    drop(release);
    delivered(&mut service, &client, &wake, true)?;
    assert!(service.cleanup_stopped());
    assert!(!service.files_retired());
    assert_eq!(source.inspect(|operation| operation.retired && operation.failed()), Some(true));
    let failure = service.failure.take().ok_or("Original escaped recovery cause missing")?;
    drop(service);
    drop(source);
    drop(metadata_reservation);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    assert!(failure.retirement_failure().is_some());
    drop(failure);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn recovery_phase_rearming_waits_for_actual_terminal_and_checked_native_cleanup()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut metadata = spec(jobs.allocate()?);
    metadata.reservation_bytes = RECOVERY_SOURCE_BYTES;
    let mut constructor =
        client.submit_blocking(&scope, metadata, |context| RecoveryService::new(&context))?;
    let completed = observe(&client, &wake, &mut constructor)?;
    let mut service = match completed.outcome {
        JobOutcome::Success(service) => service,
        _ => return Err("recovery admission failed".into()),
    };
    drop(completed.memory);
    drop(constructor);
    let source = service.source.alias();
    service.work.job =
        Some(client.submit_blocking(&scope, spec(jobs.allocate()?), move |context| {
            let fixture = DiagnosticStorageFixture::new().map_err(|_| RecoveryIssue::Failed)?;
            let mut operation = prepared(&context, &fixture).map_err(|_| RecoveryIssue::Failed)?;
            operation.next = Some(RecoveryPhase::Capture { after: Some([7; 32]) });
            *source.loan()?.operation()? = operation;
            Ok(true)
        })?);
    service.installed = true;
    assert!(service.next_step().is_err());
    assert!(service.final_scan_ready().is_err());
    delivered(&mut service, &client, &wake, false)?;
    assert!(service.next_step().is_err());
    service.submit_cleanup(&client, &scope, &jobs, Moment::from_nanos(100))?;
    delivered(&mut service, &client, &wake, true)?;
    assert_eq!(service.next_step()?, Some(RecoveryPhase::Capture { after: Some([7; 32]) }));
    assert!(!service.final_scan_ready()?);
    // No genuine input was issued by this external-disk fixture. It cannot synthesize another op.
    let mut missing = None;
    assert!(service.advance(&mut missing).is_err());
    assert!(service.delivered() && service.files_retired());
    assert_eq!(service.next_step()?, Some(RecoveryPhase::Capture { after: Some([7; 32]) }));
    service.source.release()?;
    service.work.completion = None;
    service.cleanup.completion = None;
    drop(service);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
