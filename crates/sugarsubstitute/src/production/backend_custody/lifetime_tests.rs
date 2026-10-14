// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe original helper guards, retirement budgets and external causes on one execution owner.
use super::*;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
};
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other("Original wake poisoned")),
            )
        })?;
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
/// Keep the actual helper lifetime with its deterministic external clock and wake boundary.
type TestRig = (Lifetime, Arc<TestClock>, Arc<Wake>);
fn lifetime_at(now: u64) -> Result<TestRig, Box<dyn Error>> {
    let role = Role {
        input: None,
        original: sugarsubstitute_execution::windows_backend_custody::LifecyclePipeDeadline::new(
            1, 1,
        )?,
        key: BackendCustodyKey::new(
            CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            BackendCustodySession::new(1)?,
        ),
        installation: None,
    };
    let clock = Arc::new(TestClock(AtomicU64::new(now)));
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut lifetime = Lifetime::new(role);
    lifetime.client = Some(owner.client());
    lifetime.execution = Some(owner);
    Ok((lifetime, clock, wake))
}
#[test]
fn helper_uncaptured_first_arithmetic_failure_cannot_create_later_shutdown_budget()
-> Result<(), Box<dyn Error>> {
    let (mut lifetime, clock, _wake) = lifetime_at(u64::MAX)?;
    let failure = lifetime.advance_shutdown().err().ok_or("Original arithmetic failure missing")?;
    assert!(failure.is::<TimeError>());
    assert!(lifetime.uncaptured_sampled);
    assert!(lifetime.until.is_none());
    clock.0.store(0, Ordering::Release);
    lifetime.advance_shutdown()?;
    assert!(lifetime.until.is_none());
    assert!(!lifetime.poll_shutdown()?);
    assert!(lifetime.until.is_none());
    Ok(())
}
#[test]
fn helper_external_drain_failure_keeps_original_cause_and_observes_scope_shutdown()
-> Result<(), Box<dyn Error>> {
    let (mut lifetime, _clock, _wake) = lifetime_at(0)?;
    let scope = lifetime
        .client
        .as_ref()
        .ok_or("Original execution missing")?
        .create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    assert!(!scope.snapshot().closed);
    lifetime.closing = true;
    lifetime.capture.request_cleanup();
    let cause = ExecutionError::new(
        Operation::NativeLifecycle,
        Box::new(std::io::Error::other("original completion delivery failure")),
    );
    let failure =
        lifetime.poll_retirement(Some(Err(cause))).err().ok_or("Original drain cause lost")?;
    let retained =
        failure.downcast_ref::<ExecutionError>().ok_or("Original execution type lost")?;
    assert_eq!(retained.operation(), Operation::NativeLifecycle);
    assert_eq!(
        retained.source().ok_or("Original source lost")?.to_string(),
        "original completion delivery failure"
    );
    assert!(scope.snapshot().closed);
    assert_eq!(lifetime.until, Some(Moment::from_nanos(15_000_000_000)));
    assert_eq!(scope.snapshot().queued, 0);
    assert_eq!(scope.snapshot().running, 0);
    drop(lifetime);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn active_helper_cancellation_rejects_healthy_shutdown_and_keeps_whole_original_guard()
-> Result<(), Box<dyn Error>> {
    let (mut lifetime, _clock, wake) = lifetime_at(0)?;
    let client = lifetime.client.as_ref().ok_or("Original execution missing")?.clone();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let request = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::Startup,
        class: JobClass::IoLifecycle,
        deadline: Some(Moment::from_nanos(1000)),
        reservation_bytes: 4096,
        replacement: None,
    };
    let job = client.submit_async(&scope, request, |_| async {
        std::future::pending::<Result<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>>()
            .await
    })?;
    job.cancel();
    lifetime.terminals.worker_original =
        Some(super::super::terminal::Original::new(&scope, request));
    lifetime.worker = Some(job);
    let failure = loop {
        let revision = *wake.revision.lock().map_err(|_| "Original wake poisoned")?;
        client.drain_completions(NonZeroUsize::MAX)?;
        match lifetime.observe() {
            Ok(HeadlessStep::Active) => {}
            Ok(_) => return Err("Unexpected active cancellation accepted".into()),
            Err(cause) => break cause,
        }
        let (_after, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.revision.lock().map_err(|_| "Original wake poisoned")?,
                std::time::Duration::from_secs(10),
                |current| *current == revision,
            )
            .map_err(|_| "Original wake poisoned")?;
        if timeout.timed_out() {
            return Err("Original terminal notification absent".into());
        }
    };
    assert!(failure.to_string().contains("Cancelled"));
    assert!(!lifetime.closing);
    drop(lifetime.worker.take());
    drop(lifetime.execution.take());
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(lifetime);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
