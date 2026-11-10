// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual seed/partial repository custody through central completion and worker join.
use super::service::SeedServiceIssue;
use super::*;
use crate::completion_wake::Wake;
use std::{error::Error, num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::{execution::*, incident::*, lifecycle::*};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::diagnostic_storage::fixture::*;
pub(super) type TestError = Box<dyn Error + Send + Sync>;
pub(super) struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
pub(super) fn observe<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, TestError> {
    loop {
        let before = wake.revision();
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(value) = job.try_take_completion() {
            return Ok(value);
        }
        wake.wait_after(before);
    }
}
pub(super) fn key() -> Result<LogicalStoreOperationKey, TestError> {
    Ok(LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        IncidentOperationId::new(1)?,
    ))
}
pub(super) fn seed() -> Result<IncidentSeed, TestError> {
    Ok(IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Desktop(RunKey {
            owner: key()?.owner(),
            instance: InstanceId::new(3)?,
            generation: RunGeneration::new(4)?,
        })),
        IncidentRevision::new(1)?,
        BuildIdentity::new("build_1")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::Startup,
    )?)
}
pub(super) fn prepared(
    context: &JobContext,
    fixture: &DiagnosticStorageFixture,
) -> Result<SeedOperation, TestError> {
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(key()?, context)?;
    let opening = fixture.resolve_opening(&mut storage, &operation, context)?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    Ok(SeedOperation {
        stage: SeedStage::Writer,
        repository: Some(IncidentRepository::new(storage, operation, context)?),
        opening: Some(opening),
        committed: None,
        first: None,
        retirement: None,
        memory: None,
        retired: false,
    })
}
pub(super) fn spec(id: JobId) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
        replacement: None,
    }
}
#[test]
fn seed_commit_and_opening_keep_original_eight_mib_after_actual_worker_join()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = crate::job_sequence::JobSequence::new();
    let mut job = client.submit_blocking(
        &scope,
        spec(jobs.allocate()?),
        |context| -> Result<SeedOperation, TestError> {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut operation = prepared(&context, &fixture)?;
            operation.publish(seed()?, &context)?;
            assert!(operation.committed().is_some());
            assert!(operation.failure().is_none());
            assert!(operation.memory_failure().is_none());
            assert!(operation.retire(&context));
            assert!(operation.files_retired());
            Ok(operation)
        },
    )?;
    let completion = observe(&client, &wake, &mut job)?;
    let operation = match completion.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("real seed worker did not succeed".into()),
    };
    drop(completion.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    drop(operation);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn helper_preserves_first_native_cause_and_distinct_failed_close_until_checked_retirement()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = crate::job_sequence::JobSequence::new();
    let mut job = client.submit_blocking(
        &scope,
        spec(jobs.allocate()?),
        |context| -> Result<SeedOperation, TestError> {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut operation = prepared(&context, &fixture)?;
            fixture.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
            operation.first = Some(
                operation
                    .publish(seed()?, &context)
                    .err()
                    .ok_or("partial output unexpectedly published")?,
            );
            let first = operation.failure().ok_or("first cause lost")?.code();
            assert!(operation.failure().and_then(Error::source).is_some());
            assert!(operation.committed().is_none());
            fixture.set_failure(DiagnosticFixtureFailure::Close, true);
            assert!(!operation.retire(&context));
            assert!(!operation.files_retired());
            assert!(operation.retirement_failure().is_some());
            assert_eq!(operation.failure().ok_or("first cause replaced")?.code(), first);
            assert!(context.memory().claim(1).is_err());
            fixture.set_failure(DiagnosticFixtureFailure::Close, false);
            assert!(operation.retire(&context));
            assert_eq!(operation.failure().ok_or("first cause dropped after close")?.code(), first);
            Ok(operation)
        },
    )?;
    let completion = observe(&client, &wake, &mut job)?;
    let operation = match completion.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("real partial worker did not finish".into()),
    };
    drop(completion.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    drop(operation);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn restored_source_keeps_partial_native_cause_after_worker_panic_and_real_cleanup()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = crate::job_sequence::JobSequence::new();
    let mut controller_spec = spec(jobs.allocate()?);
    controller_spec.reservation_bytes = crate::production::incident_host::CONTROLLER_BYTES;
    let mut controller =
        client.submit_blocking(&scope, controller_spec, |context| source::Source::new(&context))?;
    let completion = observe(&client, &wake, &mut controller)?;
    let source = match completion.outcome {
        JobOutcome::Success(source) => source,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("source construction failed".into()),
    };
    drop(completion.memory);
    drop(controller);
    let work = source.alias();
    let mut worker = client.submit_blocking(
        &scope,
        spec(jobs.allocate()?),
        move |context| -> Result<(), TestError> {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut loan = work.loan()?;
            *loan.operation()? = prepared(&context, &fixture)?;
            fixture.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
            let operation = loan.operation()?;
            operation.first = Some(
                operation
                    .publish(seed()?, &context)
                    .err()
                    .ok_or("partial stage was not retained")?,
            );
            std::panic::resume_unwind(Box::new(991_u64))
        },
    )?;
    let completion = observe(&client, &wake, &mut worker)?;
    match &completion.outcome {
        JobOutcome::Panicked(cause) => assert!(!cause.as_str().is_empty()),
        _ => return Err("actual worker panic was not delivered".into()),
    }
    assert_eq!(
        source.inspect(|operation| operation.failure().and_then(Error::source).is_some()),
        Some(true)
    );
    assert_eq!(source.inspect(SeedOperation::files_retired), Some(false));
    let cleanup = source.alias();
    let mut cleanup_spec = spec(jobs.allocate()?);
    cleanup_spec.reservation_bytes = 32 * 1024;
    let mut cleanup_job = client.submit_blocking(&scope, cleanup_spec, move |context| {
        let mut loan = cleanup.loan()?;
        Ok::<_, SeedServiceIssue>(loan.operation()?.retire(&context))
    })?;
    let cleanup_completion = observe(&client, &wake, &mut cleanup_job)?;
    assert!(matches!(cleanup_completion.outcome, JobOutcome::Success(true)));
    assert_eq!(source.inspect(SeedOperation::files_retired), Some(true));
    drop(completion);
    drop(cleanup_completion);
    drop(cleanup_job);
    drop(worker);
    drop(owner);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        DIAGNOSTIC_STORAGE_BYTES + crate::production::incident_host::CONTROLLER_BYTES + 32 * 1024
    );
    let final_alias = source.alias();
    drop(source);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        DIAGNOSTIC_STORAGE_BYTES + crate::production::incident_host::CONTROLLER_BYTES + 32 * 1024
    );
    drop(final_alias);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
