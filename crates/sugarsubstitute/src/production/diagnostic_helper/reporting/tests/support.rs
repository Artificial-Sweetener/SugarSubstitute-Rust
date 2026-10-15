// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own real finite execution and a deterministic external filesystem fixture.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::lifecycle::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
pub(super) type TestError = Box<dyn std::error::Error + Send + Sync>;
pub(super) type TestResult = Result<(), TestError>;
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
pub(super) struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(super) fn execution() -> Result<(ExecutionOwner, Arc<Wake>, Scope), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let scope =
        owner.client().create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    Ok((owner, wake, scope))
}
pub(super) fn spec(
    id: u64,
    bytes: u64,
) -> Result<JobSpec, sugarsubstitute_contracts::execution::IdentityError> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: bytes,
        replacement: None,
    })
}
pub(super) fn completed<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, TestError> {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(value) = job.try_take_completion() {
            return Ok(value);
        }
        if batch.remaining > 0 {
            continue;
        }
        wait(wake, before)?;
    }
}
pub(super) fn delivered(
    client: &ExecutionClient,
    wake: &Wake,
    service: &mut ReportingService,
    cleanup: bool,
) -> TestResult {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        if cleanup {
            service.capture_cleanup(&batch)?;
        } else {
            service.capture(&batch)?;
        }
        if if cleanup {
            service.cleanup.completion.is_some()
        } else {
            service.work.completion.is_some()
        } {
            return Ok(());
        }
        if batch.remaining > 0 {
            continue;
        }
        wait(wake, before)?;
    }
}
/// Observe the actual native Prepared bootstrap without installing or launching a child.
pub(super) fn prepared_parent(
    client: &ExecutionClient,
    wake: &Wake,
    source: &crate::production::reporter_role::parent_source::ReporterSource,
) -> TestResult {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        let facts = source.poll()?;
        assert!(!facts.authenticated);
        assert!(facts.process.is_none());
        if source.prepare(|_| ())?.is_some() {
            return Ok(());
        }
        if facts.failed || facts.pipe_settled {
            return Err("Actual Reporter pipe failed before preparation".into());
        }
        if batch.remaining > 0 {
            continue;
        }
        wait(wake, before)?;
    }
}
/// Drive the real closed Parent until its original worker supplies checked pipe retirement.
/// Completion wakes are causal observations; the existing wait bound only diagnoses a stalled test.
pub(super) fn retired_parent(
    client: &ExecutionClient,
    wake: &Wake,
    source: &crate::production::reporter_role::parent_source::ReporterSource,
) -> TestResult {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        let actual = source.poll_retiring()?;
        assert!(!actual.authenticated);
        assert!(actual.process.is_none());
        if actual.pipe_settled {
            return Ok(());
        }
        if batch.remaining > 0 {
            continue;
        }
        wait(wake, before)?;
    }
}
fn wait(wake: &Wake, before: u64) -> TestResult {
    let (count, timeout) = wake
        .changed
        .wait_timeout_while(
            wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
            Duration::from_secs(10),
            |count| *count == before,
        )
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if timeout.timed_out() && *count == before {
        return Err("actual reporting delivery stalled".into());
    }
    Ok(())
}
pub(super) fn delivered_view(
    client: &ExecutionClient,
    wake: &Wake,
    service: &mut ReportingService,
) -> TestResult {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        service.capture_view(&batch)?;
        if service.presentation.completion.is_some() {
            return Ok(());
        }
        if batch.remaining > 0 {
            continue;
        }
        wait(wake, before)?;
    }
}
pub(super) fn success<T, E: Into<TestError>>(
    completion: JobCompletion<T, E>,
) -> Result<T, TestError> {
    match completion.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(cause.into()),
        _ => Err("actual report worker did not succeed".into()),
    }
}
pub(super) fn original() -> Result<IncidentSummary, TestError> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Desktop(RunKey {
            owner,
            instance: InstanceId::new(3)?,
            generation: RunGeneration::new(4)?,
        })),
        IncidentRevision::new(1)?,
        BuildIdentity::new("build_1")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::Running,
    )?;
    let bytes = "original Unicode 診断".as_bytes();
    Ok(IncidentSummary::new(
        seed,
        IncidentRevision::new(2)?,
        SummaryFacts::new(
            RetentionFacts::new(IncidentUtc::from_unix_seconds(101)?, None)?,
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::RustPanic,
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            PayloadDisposition::Retained(AttachmentInventory::new(&[AttachmentFacts::new(
                AttachmentKind::Fault,
                bytes.len() as u64,
                bytes.len() as u64,
                0,
                Sha256::digest(bytes).into(),
            )?])?),
            ReportDisposition::new(0, None)?,
            SymbolOutcome::NotRequested,
        ),
    )?)
}
pub(super) fn binding(summary: &IncidentSummary) -> Result<ReportBinding, IncidentError> {
    Ok(ReportBinding::new(summary.seed().key(), summary.revision(), ReportSessionId::new(7)?))
}
pub(super) fn prepared(
    context: &JobContext,
    fixture: &DiagnosticStorageFixture,
) -> Result<ReportingOperation, TestError> {
    let summary = original()?;
    let key = summary.seed().key();
    let owner = key.subject().coordinator();
    fixture.put(
        DiagnosticObject::StoreState,
        &encode_store_state(&IncidentStoreState::Ledger(SummaryEvictions::new(
            owner.namespace,
            0,
            0,
            0,
            None,
        )?))?,
    )?;
    fixture.put(
        DiagnosticObject::Summary(key),
        &encode_summary(&StoredIncidentSummary::new(summary.clone(), Some(summary.revision()))?)?,
    )?;
    fixture.put(
        DiagnosticObject::Payload(key, summary.revision(), AttachmentKind::Fault),
        "original Unicode 診断".as_bytes(),
    )?;
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(
        LogicalStoreOperationKey::new(owner, IncidentOperationId::new(1)?),
        context,
    )?;
    let opening = fixture.resolve_opening(&mut storage, &operation, context)?;
    storage.acquire_writer(&operation, context)?;
    let mut repository = IncidentRepository::new(storage, operation, context)?;
    repository.validate_store(context)?;
    let mut result = ReportingOperation::new();
    result.repository = Some(repository);
    result.opening = Some(opening);
    Ok(result)
}

/// Feed one live global batch to the actual namespace owner and await its causal wake.
pub(super) fn entry_delivery(
    client: &ExecutionClient,
    wake: &Wake,
    owner: &mut sugarsubstitute_supervision::coordinator_entry::CoordinatorEntryOwner,
) -> Result<sugarsubstitute_supervision::coordinator_entry::EntryObservation, TestError> {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(observation) = owner.consume(&batch)? {
            return Ok(observation);
        }
        if batch.remaining > 0 {
            continue;
        }
        wait(wake, before)?;
    }
}

/// Deliver each genuine batch once while tracking the two actual external read jobs.
pub(super) fn cancelled_delivery_with_reads(
    client: &ExecutionClient,
    wake: &Wake,
    service: &mut ReportingService,
    reads: &mut [OwnedJob<(), TestError>; 2],
) -> Result<[JobCompletion<(), TestError>; 2], TestError> {
    let mut done: [Option<JobCompletion<(), TestError>>; 2] = std::array::from_fn(|_| None);
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        service.capture(&batch)?;
        for (slot, job) in done.iter_mut().zip(reads.iter_mut()) {
            if slot.is_none() {
                *slot = job.try_take_completion();
            }
        }
        if service.outcome().is_some() && done.iter().all(Option::is_some) {
            let output = [
                done[0].take().ok_or("First real read completion missing")?,
                done[1].take().ok_or("Second real read completion missing")?,
            ];
            if !output
                .iter()
                .all(|completion| matches!(completion.outcome, JobOutcome::Success(())))
            {
                return Err("Actual external read job did not succeed".into());
            }
            return Ok(output);
        }
        if batch.remaining > 0 {
            continue;
        }
        wait(wake, before)?;
    }
}
