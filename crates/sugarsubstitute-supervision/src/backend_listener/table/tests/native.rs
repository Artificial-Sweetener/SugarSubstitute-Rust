// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual SDK snapshots and retained workspace credit on the central execution owner.
use super::*;
use std::sync::{Arc, Condvar, Mutex};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec,
    Moment, Operation, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *crate::registry::lock(&self.0) += 1;
        self.1.notify_all();
        Ok(())
    }
}
#[test]
fn actual_sdk_workspace_is_funded_once_retained_after_job_and_freed_before_final_credit()
-> Result<(), Box<dyn std::error::Error>> {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(2_000_000_000)),
            reservation_bytes: 16 * 1024 * 1024,
            replacement: None,
        },
        |context| -> Result<_, RegistryError> {
            assert!(TcpOwnerWorkspace::for_output(&context).is_err());
            let body = context
                .memory()
                .claim(8 * 1024 * 1024)
                .map_err(|cause| RegistryError::new("test_body", cause))?;
            let mut storage = TcpOwnerWorkspace::new(&context)?;
            assert_eq!(
                storage.words.capacity() * size_of::<u32>(),
                TCP_OWNER_WORKSPACE_BYTES as usize
            );
            assert_eq!(storage.words.len(), storage.words.capacity());
            let address = storage.words.as_ptr();
            let first = native::read(
                &mut storage,
                &context,
                windows::Win32::NetworkManagement::IpHelper::TCP_TABLE_OWNER_PID_CONNECTIONS,
            )?;
            assert_eq!(first.len(), first[0] as usize * 6 + 1);
            let second = native::read(
                &mut storage,
                &context,
                windows::Win32::NetworkManagement::IpHelper::TCP_TABLE_OWNER_PID_LISTENER,
            )?;
            assert_eq!(second.len(), second[0] as usize * 6 + 1);
            assert_eq!(storage.words.as_ptr(), address);
            let error = context
                .memory()
                .claim(64 * 1024 + 1)
                .err()
                .ok_or_else(|| RegistryError::invalid("test_overclaim"))?;
            assert_eq!(error.available_bytes, 64 * 1024);
            let attached = context
                .memory()
                .claim(64 * 1024)
                .map_err(|cause| RegistryError::new("test_attached", cause))?;
            assert!(context.memory().claim(1).is_err());
            drop(attached);
            Ok((storage, body))
        },
    )?;
    let completion = loop {
        let revision = *crate::registry::lock(&wake.0);
        client.drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid drain")?)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        drop(
            wake.1
                .wait_while(crate::registry::lock(&wake.0), |current| *current == revision)
                .map_err(|_| "Poisoned wake")?,
        );
    };
    let JobOutcome::Success((storage, body)) = completion.outcome else {
        return Err(format!("Actual snapshot failed: {:?}", completion.outcome.kind()).into());
    };
    drop(job);
    drop(completion.memory);
    drop(completion.native_handles);
    // Terminal delivery precedes worker-context destruction; join while both returned claims remain held.
    drop(executor);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024 * 1024);
    drop(storage);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024 * 1024);
    drop(body);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
