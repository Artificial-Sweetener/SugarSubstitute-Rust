// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own actual execution admission and the external protected filesystem fixture.
use super::*;
use std::{
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;
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
pub(in crate::incident) fn admitted(
    work: impl FnOnce(JobContext) -> TestResult + Send + 'static,
) -> TestResult {
    admitted_bytes(DIAGNOSTIC_STORAGE_BYTES, work)
}
/// Admit independently budgeted passive-history tests without changing existing native budgets.
pub(in crate::incident) fn admitted_bytes(
    bytes: u64,
    work: impl FnOnce(JobContext) -> TestResult + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    run_job(&client, &scope, &wake, 1, bytes, work)?;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
/// Retain matching data from one completed worker into a distinct finite job.
/// Each job retains its own original 8MiB admission; no reservation is renewed.
pub(super) fn admitted_handoff<T: Send + 'static>(
    prepare: impl FnOnce(JobContext) -> Result<T, TestError> + Send + 'static,
    consume: impl FnOnce(JobContext, T) -> TestResult + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let first = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let second = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let retained = run_job(&client, &first, &wake, 1, DIAGNOSTIC_STORAGE_BYTES, prepare)?;
    run_job(&client, &second, &wake, 2, DIAGNOSTIC_STORAGE_BYTES, move |context| {
        consume(context, retained)
    })?;
    drop(owner);
    assert_eq!(first.snapshot().reserved_bytes, 0);
    assert_eq!(second.snapshot().reserved_bytes, 0);
    Ok(())
}
fn run_job<T: Send + 'static>(
    client: &ExecutionClient,
    scope: &Scope,
    wake: &Wake,
    id: u64,
    bytes: u64,
    work: impl FnOnce(JobContext) -> Result<T, TestError> + Send + 'static,
) -> Result<T, TestError> {
    let mut job = client.submit_blocking(
        scope,
        JobSpec {
            id: JobId::new(id)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: bytes,
            replacement: None,
        },
        work,
    )?;
    let completion = loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == old,
            )
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *count == old {
            return Err("Repository completion stalled".into());
        }
    };
    match completion.outcome {
        JobOutcome::Success(result) => {
            drop(completion.memory);
            Ok(result)
        }
        JobOutcome::Failure(cause) => {
            let mut chain = Vec::new();
            let mut current: Option<&dyn Error> = Some(cause.as_ref());
            while let Some(error) = current {
                chain.push(error.to_string());
                current = error.source();
            }
            Err(chain.join(" -> ").into())
        }
        JobOutcome::Cancelled => Err("Repository worker cancelled".into()),
        JobOutcome::TimedOut => Err("Repository worker timed out".into()),
        JobOutcome::Panicked(cause) => Err(format!("Repository worker panicked: {cause:?}").into()),
        JobOutcome::Coalesced { replacement } => {
            Err(format!("Repository worker coalesced: {replacement:?}").into())
        }
    }
}
pub(in crate::incident) fn setup(
    context: &JobContext,
) -> Result<(DiagnosticStorageFixture, IncidentRepository), TestError> {
    let fixture = DiagnosticStorageFixture::new()?;
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let key = LogicalStoreOperationKey::new(owner, IncidentOperationId::new(1)?);
    let ledger = SummaryEvictions::new(owner.namespace, 0, 0, 0, None)?;
    fixture.put(
        DiagnosticObject::StoreState,
        &encode_store_state(&IncidentStoreState::Ledger(ledger))?,
    )?;
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(key, context)?;
    storage.resolve(
        Path::new("external-repository-fixture"),
        owner.namespace,
        &operation,
        context,
    )?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    Ok((fixture, IncidentRepository::new(storage, operation, context)?))
}
