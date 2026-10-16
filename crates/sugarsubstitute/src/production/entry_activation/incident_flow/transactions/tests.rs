// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep application admission until real writer retirement, independently of execution delivery.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::{execution::*, incident::IncidentOperationId, lifecycle::*};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::diagnostic_storage::{
    DIAGNOSTIC_STORAGE_BYTES, fixture::DiagnosticStorageFixture,
};
type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
fn operation(epoch: u8, id: u64) -> TestResult<LogicalStoreOperationKey> {
    Ok(LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([epoch; 16])?,
        },
        IncidentOperationId::new(id)?,
    ))
}
#[test]
fn namespace_transaction_waits_through_main_delivery_until_actual_checked_writer_retirement()
-> TestResult {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let original = operation(2, 1)?;
    let mut admission = DiagnosticTransactions::new();
    admission.bind(original.owner())?;
    let mut reporting = None;
    let mut seed = None;
    assert!(admission.admit(DiagnosticTransactionRole::Reporting, original, &mut reporting)?);
    let mut main =
        client.submit_blocking(&scope, spec(1, DIAGNOSTIC_STORAGE_BYTES)?, move |context| {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut storage = fixture.storage(&context)?;
            let operation = storage.begin_operation(original, &context)?;
            let opening = fixture.resolve_opening(&mut storage, &operation, &context)?;
            storage.acquire_writer(&operation, &context)?;
            storage.inventory(&operation, &context)?;
            Ok::<_, Box<dyn Error + Send + Sync>>((storage, operation, opening))
        })?;
    let completed = completion(&client, &wake, &mut main)?;
    let (mut storage, original_operation, opening) = match completed.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("Actual writer preparation stopped".into()),
    };
    assert!(storage.snapshot().writer_held());
    assert!(!storage.snapshot().admission_closed());
    assert!(!admission.settled());
    let pending = operation(2, 2)?;
    assert!(!admission.admit(DiagnosticTransactionRole::Seed, pending, &mut seed)?);
    assert!(seed.is_none());
    // Independent native/control metadata still uses the same central execution boundary.
    let mut control = client
        .submit_blocking(&scope, spec(2, 4096)?, |_| Ok::<_, Box<dyn Error + Send + Sync>>(()))?;
    assert!(matches!(completion(&client, &wake, &mut control)?.outcome, JobOutcome::Success(())));
    assert!(!admission.admit(DiagnosticTransactionRole::Seed, pending, &mut seed)?);
    let mut cleanup = client.submit_blocking(&scope, spec(3, 4096)?, move |context| {
        storage.retire(&original_operation, &context)?;
        let actual = storage.snapshot();
        assert!(actual.admission_closed());
        assert!(!actual.writer_held());
        assert_eq!(actual.live_handles(), 0);
        drop((storage, original_operation, opening));
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    })?;
    assert!(matches!(completion(&client, &wake, &mut cleanup)?.outcome, JobOutcome::Success(())));
    admission.retired(original, &mut reporting)?;
    assert!(admission.admit(DiagnosticTransactionRole::Seed, pending, &mut seed)?);
    admission.retired(pending, &mut seed)?;
    assert!(admission.settled());
    drop((completed.memory, main, control, cleanup));
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn namespace_transaction_rejects_changed_owner_role_and_operation_without_releasing_original()
-> TestResult {
    let original = operation(2, 1)?;
    let mut admission = DiagnosticTransactions::new();
    admission.bind(original.owner())?;
    let mut token = None;
    assert!(admission.admit(DiagnosticTransactionRole::Recovery, original, &mut token)?);
    assert!(admission.bind(operation(3, 1)?.owner()).is_err());
    assert!(
        admission.admit(DiagnosticTransactionRole::Recovery, operation(3, 1)?, &mut None).is_err()
    );
    assert!(admission.admit(DiagnosticTransactionRole::Reporting, original, &mut token).is_err());
    assert!(
        admission.admit(DiagnosticTransactionRole::Recovery, operation(2, 2)?, &mut token).is_err()
    );
    assert!(admission.retired(operation(3, 1)?, &mut token).is_err());
    assert!(admission.retired(operation(2, 2)?, &mut token).is_err());
    admission.retired(original, &mut None)?;
    assert!(!admission.settled());
    assert!(admission.admit(DiagnosticTransactionRole::Recovery, original, &mut token)?);
    admission.retired(original, &mut token)?;
    assert!(token.is_none());
    assert!(admission.settled());
    Ok(())
}
#[test]
fn namespace_transaction_retired_token_cannot_release_fresh_generation_or_other_source_roles()
-> TestResult {
    let mut admission = DiagnosticTransactions::new();
    admission.bind(operation(2, 1)?.owner())?;
    let mut report = None;
    let mut next = None;
    assert_eq!(size_of::<DiagnosticTransaction>(), 16);
    assert_eq!(size_of::<Option<DiagnosticTransaction>>(), 16);
    let roles = [
        DiagnosticTransactionRole::Recovery,
        DiagnosticTransactionRole::Collection,
        DiagnosticTransactionRole::Symbols,
        DiagnosticTransactionRole::Reporting,
        DiagnosticTransactionRole::Export,
        DiagnosticTransactionRole::RecoveryOrigin,
        DiagnosticTransactionRole::Seed,
    ];
    for (index, role) in roles.into_iter().enumerate() {
        let key = operation(2, u64::try_from(index)? + 1)?;
        assert!(admission.admit(role, key, &mut report)?);
        assert!(!admission.admit(DiagnosticTransactionRole::Seed, operation(2, 20)?, &mut next)?);
        assert!(next.is_none());
        admission.retired(key, &mut report)?;
        // Retained report metadata does not keep source admission after checked retirement.
        assert!(admission.settled());
        assert!(admission.admit(DiagnosticTransactionRole::Seed, operation(2, 20)?, &mut next)?);
        admission.retired(operation(2, 20)?, &mut next)?;
        // Repeated close-only observation of the old owner cannot release a different active role.
        assert!(admission.admit(role, key, &mut next)?);
        admission.retired(key, &mut report)?;
        assert!(!admission.settled());
        admission.retired(key, &mut next)?;
    }
    Ok(())
}
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
struct Wake {
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
fn spec(id: u64, reservation_bytes: u64) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes,
        replacement: None,
    })
}
fn completion<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> TestResult<JobCompletion<T, E>> {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
        }
        if batch.remaining > 0 {
            continue;
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
            return Err("Actual namespace transaction boundary stalled".into());
        }
    }
}
