// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An actual central worker retains its source alias until its original terminal boundary.
use super::*;
use std::{error::Error, num::NonZeroUsize, sync::Condvar, time::Duration};
use sugarsubstitute_execution::ExecutionOwner;
type TestError = Box<dyn Error + Send + Sync>;
type TestResult<T> = Result<T, TestError>;
type TestJob<T> = OwnedJob<T, TestError>;
type TestCompletion<T> = JobCompletion<T, TestError>;
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
#[derive(Default)]
struct Gate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}
fn spec(id: u64, bytes: u64) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(60_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    })
}
fn terminal<T: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut TestJob<T>,
) -> TestResult<TestCompletion<T>> {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
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
            return Err("Original worker terminal stalled".into());
        }
    }
}
#[test]
fn recycle_concurrent_original_worker_alias_denies_reset_until_actual_return_and_join()
-> TestResult<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut prepare = client.submit_blocking(&scope, spec(1, CONTROLLER_BYTES)?, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        Ok(ReporterSource::new(&mut funding)?)
    })?;
    let prepared = terminal(&client, &wake, &mut prepare)?;
    let mut source = match prepared.outcome {
        JobOutcome::Success(source) => source,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("Original source admission did not succeed".into()),
    };
    drop(prepared.memory);
    drop(prepare);
    assert_eq!(source.close(), Err(ReporterSourceIssue::Busy));
    let pointer = {
        let mut loan = source.loan()?;
        std::ptr::from_ref(loan.state()?) as usize
    };
    let gate = Arc::new(Gate::default());
    let worker_gate = gate.clone();
    let alias = source.alias();
    let mut held = client.submit_blocking(&scope, spec(2, 32768)?, move |context| {
        let original_alias = alias;
        if context.cancellation_requested() {
            return Err("Actual alias worker cancelled".into());
        }
        let mut state = worker_gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = true;
        worker_gate.changed.notify_all();
        let (state, timeout) = worker_gate
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && !state.1 {
            return Err("Original worker alias release stalled".into());
        }
        drop(state);
        drop(original_alias);
        Ok(())
    })?;
    let (state, timeout) = gate
        .changed
        .wait_timeout_while(
            gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
            Duration::from_secs(10),
            |state| !state.0,
        )
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if timeout.timed_out() && !state.0 {
        return Err("Actual alias worker never entered".into());
    }
    drop(state);
    assert!(matches!(source.recycling_loan(), Err(ReporterSourceIssue::Busy)));
    assert!(source.loan()?.state()?.funding.is_some());
    gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1 = true;
    gate.changed.notify_all();
    let done = terminal(&client, &wake, &mut held)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())));
    drop(done);
    drop(held);
    drop(owner);
    let mut loan = source.recycling_loan()?;
    assert_eq!(std::ptr::from_ref(loan.state()?) as usize, pointer);
    assert!(loan.state()?.funding.is_some());
    drop(loan);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(source);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
