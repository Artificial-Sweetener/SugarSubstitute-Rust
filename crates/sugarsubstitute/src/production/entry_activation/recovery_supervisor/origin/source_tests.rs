// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real reservation and restored native custody without fabricating a genuine grant.
use super::super::test_support::*;
use super::*;
use crate::job_sequence::JobSequence;
use sugarsubstitute_supervision::diagnostic_storage::fixture::{
    DiagnosticFixtureFailure, DiagnosticStorageFixture,
};
pub(in crate::production::entry_activation::recovery_supervisor::origin) fn source(
    client: &ExecutionClient,
    scope: &Scope,
    wake: &Wake,
    jobs: &JobSequence,
) -> Result<Source, TestError> {
    let execution = scope.owner_id();
    let mut job = client.submit_blocking(
        scope,
        spec(jobs.allocate()?, 4 * 1024 * 1024),
        move |context| -> Result<Source, TestError> {
            let claim = context.memory().claim(storage_bytes())?;
            Ok(Source {
                shared: Some(Arc::new(Shared {
                    execution,
                    state: Mutex::new(State {
                        input: None,
                        operation: Some(Box::new(Publication::new())),
                        used: false,
                        completed: None,
                    }),
                })),
                charge: Some(Arc::new(Charge { claim })),
            })
        },
    )?;
    success(observe(client, wake, &mut job)?)
}
/// Install the actual fixture repository under the real source loan, retaining it through unwind.
/// This test-only owner adapter exposes no production mutable slot or grant constructor.
pub(in crate::production::entry_activation::recovery_supervisor::origin) fn prepared_loan(
    source: &Source,
    context: &JobContext,
    fixture: &DiagnosticStorageFixture,
) -> Result<Loan, TestError> {
    let mut loan = source.loan(true)?;
    **loan.operation.as_mut().ok_or("original slot missing")? =
        operation::tests::prepared(context, fixture)?;
    Ok(loan)
}
#[test]
fn last_source_alias_retains_actual_projection_after_producer_join() -> Result<(), TestError> {
    let (owner, client, wake, scope) = owner()?;
    let jobs = JobSequence::new();
    let source = source(&client, &scope, &wake, &jobs)?;
    assert!(storage_bytes() <= 16 * 1024);
    let alias = source.alias()?;
    drop(source);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 4 * 1024 * 1024);
    drop(alias);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn exact_loan_restores_slot_and_never_admits_another_begin() -> Result<(), TestError> {
    let (_owner, client, wake, scope) = owner()?;
    let jobs = JobSequence::new();
    let source = source(&client, &scope, &wake, &jobs)?;
    let first = source.loan(true)?;
    let pointer = first.operation.as_deref().ok_or("operation missing")? as *const Publication;
    assert!(matches!(source.loan(false), Err(OriginIssue::Busy)));
    drop(first);
    assert!(matches!(source.loan(true), Err(OriginIssue::Started)));
    let cleanup = source.loan(false)?;
    assert_eq!(
        cleanup.operation.as_deref().ok_or("original operation lost")? as *const Publication,
        pointer
    );
    drop(cleanup);
    assert!(!source.retired());
    Ok(())
}
#[test]
fn actual_worker_panic_preserves_repository_and_failed_close_cause_until_checked_cleanup()
-> Result<(), TestError> {
    let (owner, client, wake, scope) = owner()?;
    let jobs = JobSequence::new();
    let source = source(&client, &scope, &wake, &jobs)?;
    let fixture = Arc::new(DiagnosticStorageFixture::new()?);
    let worker_source = source.alias()?;
    let external = fixture.clone();
    let mut worker = client.submit_blocking(
        &scope,
        spec(jobs.allocate()?, DIAGNOSTIC_STORAGE_BYTES),
        move |context| -> Result<(), TestError> {
            let mut loan = worker_source.loan(true)?;
            **loan.operation.as_mut().ok_or("original slot missing")? =
                operation::tests::prepared(&context, &external)?;
            std::panic::resume_unwind(Box::new(991_u64))
        },
    )?;
    let done = observe(&client, &wake, &mut worker)?;
    assert!(matches!(done.outcome, JobOutcome::Panicked(_)));
    drop(done);
    drop(worker);
    assert!(!source.retired());
    assert!(fixture.snapshot().live_outputs() > 0);
    fixture.set_failure(DiagnosticFixtureFailure::Close, true);
    let cleanup_source = source.alias()?;
    let external = fixture.clone();
    let mut cleanup = client.submit_blocking(
        &scope,
        spec(jobs.allocate()?, 32 * 1024),
        move |context| -> Result<(), TestError> {
            let mut loan = cleanup_source.loan(false)?;
            assert_eq!(loan.retire(&context), Err(OriginIssue::Cleanup));
            assert!(!loan.operation.as_ref().ok_or("native custody lost")?.retired());
            external.set_failure(DiagnosticFixtureFailure::Close, false);
            assert_eq!(loan.retire(&context), Ok(true));
            assert!(loan.operation.as_ref().ok_or("cause owner lost")?.failure(true).is_some());
            assert!(loan.operation.as_ref().ok_or("completion owner lost")?.completed().is_none());
            Ok(())
        },
    )?;
    success(observe(&client, &wake, &mut cleanup)?)?;
    drop(cleanup);
    assert!(source.retired());
    let cause = source.failure(true).ok_or("original cleanup cause missing")?;
    assert_eq!(cause.category(), OriginFailureCategory::RepositoryCleanup);
    assert!(Error::source(&cause).and_then(Error::source).is_some());
    assert_eq!(fixture.snapshot().live_outputs(), 0);
    drop(fixture);
    drop(source);
    drop(owner);
    assert!(scope.snapshot().reserved_bytes >= 4 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES);
    drop(cause);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn recovery_origin_repository_cleanup_closes_shared_journal_read_across_distinct_scope()
-> Result<(), TestError> {
    use sugarsubstitute_contracts::incident::{IncidentOperationId, LogicalStoreOperationKey};
    use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};
    use sugarsubstitute_supervision::diagnostic_storage::{
        DiagnosticObject, DiagnosticStorageFailureCode,
    };
    let (owner, client, wake, scope) = owner()?;
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    assert_ne!(scope.id(), cleanup.id());
    let jobs = JobSequence::new();
    let source = source(&client, &scope, &wake, &jobs)?;
    let fixture = Arc::new(DiagnosticStorageFixture::new()?);
    let worker_source = source.alias()?;
    let external = fixture.clone();
    let mut worker = client.submit_blocking(
        &scope,
        spec(jobs.allocate()?, DIAGNOSTIC_STORAGE_BYTES),
        move |context| -> Result<_, TestError> {
            let mut loan = worker_source.loan(true)?;
            let (publication, read) = operation::tests::prepared_read(&context, &external)?;
            **loan.operation.as_mut().ok_or("Original publication slot missing")? = publication;
            Ok(read)
        },
    )?;
    let read = success(observe(&client, &wake, &mut worker)?)?;
    let facts = read.facts();
    assert_eq!(fixture.snapshot().live_outputs(), 1);
    let external = fixture.clone();
    let mut normal_close = client.submit_blocking(
        &cleanup,
        spec(jobs.allocate()?, 32 * 1024),
        move |context| -> Result<_, TestError> {
            let mut read = read;
            let before = external.snapshot().live_outputs();
            let cause = read
                .retire(&context)
                .err()
                .ok_or("Normal read close accepted foreign cleanup scope")?;
            assert_eq!(cause.code(), DiagnosticStorageFailureCode::InvalidIdentity);
            assert_eq!(external.snapshot().live_outputs(), before);
            Ok(read)
        },
    )?;
    let read = success(observe(&client, &wake, &mut normal_close)?)?;
    let mut service = OriginPublication::from_source(source.alias()?);
    assert!(!service.close(&client, &cleanup, &jobs)?);
    capture_until(&client, &wake, |batch| {
        service.capture(batch)?;
        Ok(service.close(&client, &cleanup, &jobs)?)
    })?;
    assert!(service.files_retired());
    assert!(source.retired());
    assert!(service.failure().is_none());
    let loan = source.loan(false)?;
    let actual = operation::tests::snapshot(
        loan.operation.as_deref().ok_or("Retired publication missing")?,
    )?;
    assert!(actual.admission_closed());
    assert!(!actual.writer_held());
    assert_eq!(actual.reads(), 0);
    assert_eq!(actual.live_handles(), 0);
    drop(loan);
    assert_eq!(fixture.snapshot().live_outputs(), 0);
    let object = DiagnosticObject::Journal(LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        IncidentOperationId::new(1)?,
    ));
    assert_eq!(fixture.snapshot().active_identity(object), Some(facts));
    drop((worker, normal_close));
    drop(owner);
    assert!(scope.snapshot().reserved_bytes >= 4 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES);
    drop((service, source, fixture));
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    drop(read);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
