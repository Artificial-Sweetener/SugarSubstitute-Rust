// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the real submission endpoint samples its exact injected owner epoch.
use crate::runtime_test_support::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
#[path = "runtime_clock_wait_tests.rs"]
mod wait_tests;

#[test]
fn client_timestamp_reads_original_owner_epoch_without_admission_or_storage() -> TestResult {
    let rig = Rig::new()?;
    let before = rig.scope.snapshot();
    assert_eq!(rig.client.now()?, Moment::from_nanos(0));
    rig.clock.advance(43_000_017);
    assert_eq!(rig.client.now()?, Moment::from_nanos(43_000_017));
    assert_eq!(rig.client.now()?, Moment::from_nanos(43_000_017));
    assert_eq!(rig.scope.snapshot(), before);
    Ok(())
}

struct InitialFailure;
impl Clock for InitialFailure {
    fn now(&self) -> Moment {
        std::panic::resume_unwind(Box::new("initial owner clock failure".to_owned()));
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[test]
fn initial_clock_failure_returns_original_cause_before_runtime_or_workers_exist() -> TestResult {
    use std::error::Error;
    let failure = crate::ExecutionOwner::new(Arc::new(InitialFailure), Arc::new(Wake::default()))
        .err()
        .ok_or("Initial clock panic constructed an owner")?;
    assert!(
        failure
            .source()
            .is_some_and(|cause| cause.to_string().contains("initial owner clock failure"))
    );
    Ok(())
}

struct ToggleClock {
    clock: Arc<ManualClock>,
    fail: AtomicBool,
    permanent: AtomicBool,
    calls_after_failure: AtomicU64,
    caller: std::thread::ThreadId,
    worker: std::sync::Mutex<Option<std::thread::ThreadId>>,
    poll_failure: AtomicBool,
}
impl Clock for ToggleClock {
    fn now(&self) -> Moment {
        if self.permanent.load(Ordering::Acquire) {
            self.calls_after_failure.fetch_add(1, Ordering::AcqRel);
            std::panic::resume_unwind(Box::new("permanent provider failure".to_owned()));
        }
        let current = std::thread::current().id();
        let target = *self.worker.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if (current == self.caller || target == Some(current))
            && self.fail.swap(false, Ordering::AcqRel)
        {
            std::panic::resume_unwind(Box::new("timestamp boundary failure".to_owned()));
        }
        ClockPort(self.clock.clone()).now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        if self.permanent.load(Ordering::Acquire) {
            self.calls_after_failure.fetch_add(1, Ordering::AcqRel);
            std::panic::resume_unwind(Box::new("permanent provider timer failure".to_owned()));
        }
        if deadline == Moment::from_nanos(777) && self.poll_failure.load(Ordering::Acquire) {
            return Box::pin(std::future::poll_fn(|_| {
                std::panic::resume_unwind(Box::new(
                    "original provider future poll failure".to_owned(),
                ));
            }));
        }
        ClockPort(self.clock.clone()).sleep_until(deadline)
    }
}
impl ToggleClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            clock: Arc::new(ManualClock::default()),
            fail: AtomicBool::new(false),
            permanent: AtomicBool::new(false),
            calls_after_failure: AtomicU64::new(0),
            caller: std::thread::current().id(),
            worker: std::sync::Mutex::new(None),
            poll_failure: AtomicBool::new(false),
        })
    }
}

#[test]
fn worker_first_clock_panic_preserves_panicked_completion_then_retires_without_provider()
-> TestResult {
    worker_clock_failure_observation(FailureObservationOrder::CompletionFirst)
}

#[test]
fn worker_clock_failure_is_observed_before_completion_without_losing_retirement() -> TestResult {
    worker_clock_failure_observation(FailureObservationOrder::FailureFirst)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FailureObservationOrder {
    CompletionFirst,
    FailureFirst,
}

fn worker_clock_failure_observation(order: FailureObservationOrder) -> TestResult {
    use std::error::Error;
    let clock = ToggleClock::new();
    clock.clock.advance(17);
    let wake = Arc::new(Wake::default());
    let mut owner = crate::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let worker_clock = clock.clone();
    let failed = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let worker_failed = failed.clone();
    let worker_release = release.clone();
    let mut request = spec(1, JobClass::InfrastructureOrdinary, 4096)?;
    request.deadline = Some(Moment::from_nanos(100));
    let mut job = client.submit_blocking(&scope, request, move |context| {
        *worker_clock.worker.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(std::thread::current().id());
        worker_clock.fail.store(true, Ordering::Release);
        let observation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| context.now()));
        worker_failed.open();
        if order == FailureObservationOrder::FailureFirst {
            worker_release.wait().map_err(|cause| cause.to_string())?;
        }
        match observation {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(_) => Err::<(), _>("The actual worker clock did not fail".to_owned()),
        }
    })?;
    if order == FailureObservationOrder::FailureFirst {
        failed.wait()?;
        assert!(job.try_take_completion().is_none());
        let cause = client
            .drain_completions(NonZeroUsize::MIN)
            .err()
            .ok_or("Original failure was not retained before completion")?;
        assert!(
            cause
                .source()
                .is_some_and(|source| source.to_string().contains("timestamp boundary failure"))
        );
        release.open();
    }
    let completion = loop {
        let observed = wake.count();
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        wake.wait_after(observed)?;
    };
    assert!(matches!(completion.outcome, JobOutcome::Panicked(_)));
    clock.permanent.store(true, Ordering::Release);
    let sticky = client.now().err().ok_or("Worker clock failure was not latched")?;
    assert!(
        sticky
            .source()
            .is_some_and(|cause| cause.to_string().contains("timestamp boundary failure"))
    );
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    owner.request_shutdown(Moment::from_nanos(17));
    if order == FailureObservationOrder::CompletionFirst {
        let cause = owner
            .poll_shutdown()
            .err()
            .ok_or("Taking the completion erased its independent failure notice")?;
        assert!(
            cause
                .source()
                .is_some_and(|source| source.to_string().contains("timestamp boundary failure"))
        );
    }
    assert!(!owner.poll_shutdown()?.deadline_expired);
    drop(completion);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(clock.calls_after_failure.load(Ordering::Acquire), 0);
    assert_eq!(clock.clock.expired_sleep_calls.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn client_timestamp_preserves_supplied_clock_panic_without_fabricating_metadata() -> TestResult {
    use std::error::Error;
    let clock = ToggleClock::new();
    let owner = crate::ExecutionOwner::new(clock.clone(), Arc::new(Wake::default()))?;
    let client = owner.client();
    clock.fail.store(true, Ordering::Release);
    let cause = client.now().err().ok_or("Panicking clock returned metadata")?;
    assert!(
        cause
            .source()
            .is_some_and(|source| source.to_string().contains("timestamp boundary failure"))
    );
    // An already admitted worker sample may still be returning from the provider.
    // Joining the owner distinguishes that custody from a fresh post-failure call.
    drop(owner);
    clock.permanent.store(true, Ordering::Release);
    let sticky = client.now().err().ok_or("Failed metadata returned a cached timestamp")?;
    assert!(
        sticky
            .source()
            .is_some_and(|source| source.to_string().contains("timestamp boundary failure"))
    );
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    assert_eq!(clock.calls_after_failure.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn permanent_provider_failure_retains_actual_work_until_cancelled_completion_and_worker_join()
-> TestResult {
    let clock = ToggleClock::new();
    clock.clock.advance(17);
    let wake = Arc::new(Wake::default());
    let mut owner = crate::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let mut request = spec(1, JobClass::InfrastructureOrdinary, 4096)?;
    request.deadline = Some(Moment::from_nanos(100));
    let mut job = client.submit_blocking(&scope, request, move |_| {
        worker_entered.open();
        worker_release.wait().map_err(|cause| cause.to_string())
    })?;
    entered.wait()?;
    clock.fail.store(true, Ordering::Release);
    assert!(client.now().is_err());
    clock.permanent.store(true, Ordering::Release);
    let _failure = client
        .drain_completions(NonZeroUsize::MIN)
        .err()
        .ok_or("Owner failure was not reported")?;
    owner.request_shutdown(Moment::from_nanos(17));
    let pending = owner.poll_shutdown()?;
    assert!(!pending.settled);
    assert!(!pending.deadline_expired);
    assert_eq!(pending.unresolved_job_ids, vec![request.id]);
    assert!(job.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    assert!(matches!(
        client.submit_async(&scope, spec(2, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<(), ()>(())
        }),
        Err(AdmissionError::ShuttingDown)
    ));
    release.open();
    let completion = loop {
        let observed = wake.count();
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        wake.wait_after(observed)?;
    };
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(completion.queue_time, Duration::ZERO);
    assert_eq!(completion.execution_time, Duration::ZERO);
    drop(completion);
    drop(job);
    // Actual owner destruction joins all retained workers after finite native
    // work returns; it does not use a new provider sample or fabricated expiry.
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(clock.calls_after_failure.load(Ordering::Acquire), 0);
    assert_eq!(clock.clock.expired_sleep_calls.load(Ordering::Acquire), 0);
    Ok(())
}
