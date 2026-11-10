// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove manual time completes waits and deadlines only when a test reaches them.
use super::*;
use crate::{
    ExecutionClient, ExecutionOwner,
    runtime_test_support::{TestResult, Wake, spec},
};
use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
    task::Wake as TaskWake,
};
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobCompletion, JobOutcome, OwnedJob, ScopeId, ScopeSpec,
};

#[derive(Default)]
struct Wakes(AtomicUsize);
impl TaskWake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn poll(wait: &mut WaitFuture, wakes: &Arc<Wakes>) -> Poll<()> {
    let waker = Waker::from(wakes.clone());
    wait.as_mut().poll(&mut Context::from_waker(&waker))
}

fn moment(millis: u64) -> Moment {
    Moment::from_nanos(millis * 1_000_000)
}

#[test]
fn a_wait_completes_exactly_when_time_reaches_its_deadline() -> TestResult {
    let clock = ManualClock::new();
    let wakes = Arc::new(Wakes::default());
    let mut wait = clock.sleep_until(moment(100));
    assert_eq!(poll(&mut wait, &wakes), Poll::Pending);
    assert_eq!(clock.sleepers(), 1);
    clock.advance(Duration::from_millis(99))?;
    assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
    assert_eq!(poll(&mut wait, &wakes), Poll::Pending);
    clock.advance(Duration::from_millis(1))?;
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert_eq!(poll(&mut wait, &wakes), Poll::Ready(()));
    assert_eq!(clock.sleepers(), 0);
    Ok(())
}

#[test]
fn a_deadline_already_reached_completes_on_the_first_poll() {
    let clock = ManualClock::starting_at(moment(5));
    let wakes = Arc::new(Wakes::default());
    let mut wait = clock.sleep_until(moment(5));
    assert_eq!(poll(&mut wait, &wakes), Poll::Ready(()));
    assert_eq!(clock.sleepers(), 0);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
}

#[test]
fn dropping_a_wait_releases_its_registration() {
    let clock = ManualClock::new();
    let wakes = Arc::new(Wakes::default());
    let mut wait = clock.sleep_until(moment(10));
    assert_eq!(poll(&mut wait, &wakes), Poll::Pending);
    assert_eq!(clock.sleepers(), 1);
    drop(wait);
    assert_eq!(clock.sleepers(), 0);
}

#[test]
fn one_advance_wakes_every_due_wait_and_leaves_later_ones_parked() -> TestResult {
    let clock = ManualClock::new();
    let wakes = Arc::new(Wakes::default());
    let mut waits =
        [moment(10), moment(20), moment(30)].map(|deadline| clock.sleep_until(deadline));
    for wait in &mut waits {
        assert_eq!(poll(wait, &wakes), Poll::Pending);
    }
    clock.advance_to(moment(20))?;
    assert_eq!(wakes.0.load(Ordering::SeqCst), 2);
    assert_eq!(clock.sleepers(), 1);
    assert_eq!(poll(&mut waits[0], &wakes), Poll::Ready(()));
    assert_eq!(poll(&mut waits[1], &wakes), Poll::Ready(()));
    assert_eq!(poll(&mut waits[2], &wakes), Poll::Pending);
    Ok(())
}

#[test]
fn time_never_regresses_and_an_unrepresentable_advance_changes_nothing() -> TestResult {
    let clock = ManualClock::starting_at(moment(50));
    assert_eq!(clock.advance_to(moment(49)), Err(TimeError::Regression));
    assert_eq!(clock.advance(Duration::MAX), Err(TimeError::Overflow));
    assert_eq!(clock.now(), moment(50));
    clock.advance_to(moment(50))?;
    clock.advance(Duration::from_millis(1))?;
    assert_eq!(clock.now(), moment(51));
    Ok(())
}

fn settled<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> TestResult<JobCompletion<T, E>> {
    loop {
        let observed = wake.count();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        wake.wait_after(observed)?;
    }
}

#[test]
fn an_owned_job_waits_on_manual_time_and_resumes_when_the_test_reaches_its_deadline() -> TestResult
{
    let clock = ManualClock::new();
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let resume = moment(250);
    let mut job = client.submit_async(
        &scope,
        spec(1, JobClass::IoOrdinary, 0)?,
        move |context| async move {
            context.sleep_until(resume).await;
            Ok::<_, ()>(context.now())
        },
    )?;
    clock.wait_for_sleeper(resume);
    assert!(job.try_take_completion().is_none());
    clock.advance_to(resume)?;
    let completion = settled(&client, &wake, &mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(at) if at == resume));
    Ok(())
}

#[test]
fn a_job_deadline_expires_only_when_the_test_advances_to_it() -> TestResult {
    let clock = ManualClock::new();
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let deadline = moment(10_000);
    let mut bounded = spec(1, JobClass::IoOrdinary, 0)?;
    bounded.deadline = Some(deadline);
    let mut job = client.submit_async(&scope, bounded, |_| async {
        std::future::pending::<()>().await;
        Ok::<_, ()>(())
    })?;
    clock.wait_for_sleeper(deadline);
    clock.advance_to(moment(9_999))?;
    client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
    assert!(job.try_take_completion().is_none());
    clock.advance_to(deadline)?;
    let completion = settled(&client, &wake, &mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::TimedOut));
    Ok(())
}
