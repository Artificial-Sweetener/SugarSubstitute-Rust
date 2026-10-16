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
