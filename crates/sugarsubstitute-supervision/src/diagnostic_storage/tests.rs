// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise storage through actual execution admission and private external fakes.
#[path = "allocation/tests.rs"]
mod allocation_tests;
mod binding;

#[path = "capture_intent/error_tests.rs"]
mod capture_error;
mod capture_read;
mod clocks;
mod custody;
mod disposition;
mod fixture;
mod genesis;
mod group_inventory;
mod inventory;
mod migration;
mod payload_discard;
mod recovery;
mod retirement_provider;
use super::fixture::boundary::Filesystem;
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Condvar,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::{
    execution::*,
    incident::*,
    lifecycle::{CoordinatorEpoch, NamespaceId},
};
use sugarsubstitute_execution::ExecutionOwner;
type TestError = Box<dyn std::error::Error + Send + Sync>;
type TestResult = Result<(), TestError>;
struct TestClock(AtomicU64);
impl super::fixture::BoundaryClock for TestClock {
    fn observe(&self, value: u64) {
        self.0.store(value, Ordering::SeqCst);
    }
}
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
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
fn admitted(
    work: impl FnOnce(JobContext, Arc<TestClock>) -> TestResult + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        },
        move |context| work(context, clock),
    )?;
    let outcome = observe_job(&client, &wake, &mut job)?;
    match outcome {
        JobOutcome::Success(()) => (),
        other => return Err(format!("Diagnostic CPU test failed: {other:?}").into()),
    }
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
fn observe_job(
    client: &sugarsubstitute_execution::ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<(), TestError>,
) -> Result<JobOutcome<(), TestError>, TestError> {
    let completion = loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(value) = job.try_take_completion() {
            break value;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(value) = job.try_take_completion() {
            break value;
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
            return Err("Diagnostic CPU completion stalled".into());
        }
    };
    drop(completion.memory);
    Ok(completion.outcome)
}
fn owner_key(epoch: u8) -> Result<CoordinatorKey, TestError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([epoch; 16])?,
    })
}
fn key(id: u64) -> Result<LogicalStoreOperationKey, TestError> {
    Ok(LogicalStoreOperationKey::new(owner_key(2)?, IncidentOperationId::new(id)?))
}
fn incident() -> Result<IncidentKey, TestError> {
    Ok(IncidentKey::new(IncidentSubject::Supervisor(owner_key(2)?)))
}
fn setup(
    context: &JobContext,
    filesystem: Filesystem,
) -> Result<(DiagnosticStorage, DiagnosticStorageOperation), TestError> {
    let memory = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?;
    let allocation = allocation::owner(memory)?;
    let filesystem = filesystem.retain_claim(&allocation)?;
    let mut storage =
        DiagnosticStorage::from_port(allocation, OwnedPort::new(Box::new(filesystem), None))?;
    let operation = storage.begin_operation(key(1)?, context)?;
    storage.resolve(
        Path::new("ignored-by-external-fake"),
        operation.key.owner().namespace,
        &operation,
        context,
    )?;
    storage.acquire_writer(&operation, context)?;
    assert_eq!(
        storage.inventory(&operation, context)?.status(),
        DiagnosticInventoryStatus::Complete
    );
    Ok((storage, operation))
}
