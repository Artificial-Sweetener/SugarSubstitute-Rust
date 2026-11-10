// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise owned admission and retirement with a controlled compositor boundary.

use super::*;
use crate::completion_wake::{Gate, Wake};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Clock, Generation, ScopeId, WaitFuture},
    startup::LaunchId,
};
use sugarsubstitute_execution::ExecutionOwner;

/// Completions delivered per drain, matching the host's per-poll bound.
const DRAIN_BOUND: NonZeroUsize = NonZeroUsize::MIN.saturating_add(31);

#[derive(Default)]
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

/// Deliver completions until composition work reports that its retained main may be replaced.
fn advance_until_idle(
    wake: &Wake,
    client: &ExecutionClient,
    work: &mut CompositionWork,
    now: Moment,
) -> Result<(), Box<dyn Error>> {
    wake.until(|| {
        let step = (|| -> Result<bool, Box<dyn Error>> {
            client.drain_completions(DRAIN_BOUND)?;
            work.advance(now)?;
            Ok(work.active.is_none())
        })();
        step.map(|idle| idle.then_some(())).transpose()
    })
}

/// Deliver completions until composition work has settled every retained wait.
fn settle(
    wake: &Wake,
    client: &ExecutionClient,
    work: &mut CompositionWork,
    now: Moment,
) -> Result<(), Box<dyn Error>> {
    wake.until(|| {
        let step = (|| -> Result<bool, Box<dyn Error>> {
            client.drain_completions(DRAIN_BOUND)?;
            work.settled(now)
        })();
        step.map(|settled| settled.then_some(())).transpose()
    })
}

/// Release a held worker when the test leaves scope, so a failed assertion cannot strand the
/// executor that must join it.
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}

fn identity(navigation: u64) -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: super::super::policy::tests::test_run()?,
        launch: LaunchId::new(1)?,
        navigation: Generation::new(navigation)?,
    })
}

/// A saturated reservation is retried with a new job identity, without changing its deadline.
#[test]
fn retained_wait_survives_busy_admission_and_main_shutdown() -> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::with_memory_capacity(
        Arc::new(TestClock::default()),
        wake.clone(),
        NonZeroU64::new(4 * 1024 * 1024).ok_or("Capacity absent")?,
    )?;
    let client = owner.client();
    let jobs = JobSequence::new();
    let mut work =
        CompositionWork::new(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, jobs.clone());
    work.start(client.clone())?;
    let main = identity(1)?;
    work.register(main)?;
    let (_, foreign_wait) = CompositionFence::fixture();
    let mut foreign = Some(foreign_wait);
    assert!(work.offer(identity(2)?, &mut foreign, Moment::from_nanos(0), None).is_err());
    assert!(foreign.is_some());
    drop(foreign);

    let blocker = JobSpec {
        id: jobs.allocate()?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: None,
        reservation_bytes: 4 * 1024 * 1024,
        replacement: None,
    };
    let mut job =
        client.submit_blocking(work.scope.as_ref().ok_or("Scope absent")?, blocker, |context| {
            Ok::<_, io::Error>(context.memory().clone())
        })?;
    let completion = wake.completion(&client, &mut job)?;
    let JobOutcome::Success(held) = completion.outcome else {
        return Err("Reservation job failed".into());
    };
    drop(completion.memory);
    drop(job);

    let (fence, wait) = CompositionFence::fixture();
    let mut input = Some(wait);
    assert!(work.offer(main, &mut input, Moment::from_nanos(0), None)?);
    assert!(input.is_none());
    assert!(work.pending.is_some());
    let original_deadline = work.deadline();
    assert!(!fence.completed());
    drop(held);

    let later = JobSpec { id: jobs.allocate()?, reservation_bytes: 64 * 1024, ..blocker };
    let mut later_job =
        client.submit_blocking(work.scope.as_ref().ok_or("Scope absent")?, later, |_| {
            Ok::<_, io::Error>(())
        })?;
    let later_completion = wake.completion(&client, &mut later_job)?;
    assert!(matches!(later_completion.outcome, JobOutcome::Success(())));
    drop(later_completion);
    drop(later_job);
    work.advance(Moment::from_nanos(0))?;
    assert_eq!(original_deadline, Some(Moment::from_nanos(30_000_000_000)));
    assert_eq!(work.active.as_ref().ok_or("Wait was not admitted")?.job.spec().id.get(), 4);
    work.close();
    assert!(!work.settled(Moment::from_nanos(0))?);
    assert!(work.main_retired(identity(2)?).is_err());
    advance_until_idle(&wake, &client, &mut work, Moment::from_nanos(0))?;
    assert!(fence.completed());
    work.main_retired(main)?;
    settle(&wake, &client, &mut work, Moment::from_nanos(0))?;
    work.finish(Moment::from_nanos(0))?;
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(10_000_000_000))?;
    Ok(())
}

/// A deadline reports failure without treating a still-running native wait as complete.
#[test]
fn expired_running_wait_keeps_its_main_until_actual_completion() -> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let jobs = JobSequence::new();
    let mut work =
        CompositionWork::new(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, jobs.clone());
    work.start(client.clone())?;
    let main = identity(1)?;
    work.register(main)?;
    let (fence, input) = CompositionFence::fixture();
    let mut input = Some(input);
    let hold = Arc::new(Gate::default());
    let release = Release(hold.clone());
    let worker = hold.clone();
    let spec = JobSpec {
        id: jobs.allocate()?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(30)),
        reservation_bytes: 64 * 1024,
        replacement: None,
    };
    let job = client.submit_blocking_owned_input(
        work.scope.as_ref().ok_or("Scope absent")?,
        spec,
        &mut input,
        move |input, context| {
            worker.hold();
            input.wait(&context)
        },
    )?;
    work.active = Some(Active {
        fence: fence.clone(),
        job,
        deadline: Moment::from_nanos(10),
        expiry_reported: false,
    });
    hold.wait_entered();
    clock.0.store(10, Ordering::SeqCst);
    assert!(work.advance(Moment::from_nanos(10)).is_err());
    assert!(!fence.completed());
    assert!(!fence.failed());
    work.close();
    assert!(!work.settled(Moment::from_nanos(10))?);
    assert!(work.main_retired(main).is_err());
    release.0.release();
    advance_until_idle(&wake, &client, &mut work, Moment::from_nanos(10))?;
    assert!(fence.completed());
    work.main_retired(main)?;
    settle(&wake, &client, &mut work, Moment::from_nanos(10))?;
    work.finish(Moment::from_nanos(10))?;
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(10_000_000_000))?;
    Ok(())
}

/// A completed native fence does not make its main replaceable until execution is terminal.
#[test]
fn replacement_waits_for_terminal_outcome_after_native_retirement() -> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let jobs = JobSequence::new();
    let mut work =
        CompositionWork::new(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, jobs.clone());
    work.start(client.clone())?;
    let main = identity(1)?;
    let replacement = identity(2)?;
    work.register(main)?;

    let (fence, wait) = CompositionFence::fixture();
    let mut input = Some(wait);
    let hold = Arc::new(Gate::default());
    let release = Release(hold.clone());
    let worker = hold.clone();
    let spec = JobSpec {
        id: jobs.allocate()?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(30)),
        reservation_bytes: 64 * 1024,
        replacement: None,
    };
    let job = client.submit_blocking_owned_input(
        work.scope.as_ref().ok_or("Scope absent")?,
        spec,
        &mut input,
        move |input, context| {
            input.wait(&context)?;
            worker.hold();
            Ok(())
        },
    )?;
    work.active = Some(Active {
        fence: fence.clone(),
        job,
        deadline: Moment::from_nanos(30),
        expiry_reported: false,
    });
    hold.wait_entered();
    assert!(fence.completed());

    work.main_retired(main)?;
    assert!(!work.register(replacement)?);

    clock.0.store(31, Ordering::SeqCst);
    release.0.release();
    advance_until_idle(&wake, &client, &mut work, Moment::from_nanos(31))?;
    assert!(work.register(replacement)?);
    work.close();
    work.main_retired(replacement)?;
    settle(&wake, &client, &mut work, Moment::from_nanos(31))?;
    work.finish(Moment::from_nanos(31))?;

    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(10_000_000_000))?;
    Ok(())
}

/// An expired admission never creates false completion or loses the retained native input.
#[test]
fn expired_wait_cannot_retire_the_registered_main() -> Result<(), Box<dyn Error>> {
    let jobs = JobSequence::new();
    let mut work = CompositionWork::new(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, jobs);
    let main = identity(1)?;
    work.main = Some(main);
    let (fence, input) = CompositionFence::fixture();
    work.pending =
        Some(Pending { input: Some(input), fence: fence.clone(), deadline: Moment::from_nanos(1) });
    assert!(work.advance(Moment::from_nanos(1)).is_err());
    assert!(fence.failed());
    assert!(!fence.completed());
    assert!(work.pending.as_ref().is_some_and(|pending| pending.input.is_some()));
    assert!(work.main_retired(main).is_err());
    drop(work.pending.take());
    work.main = None;
    work.close();
    work.finish(Moment::from_nanos(1))?;
    Ok(())
}

/// Closing before startup owns no native actor or worker scope.
#[test]
fn unstarted_composition_closes_without_a_native_main() -> Result<(), Box<dyn Error>> {
    let mut work = CompositionWork::new(
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
        JobSequence::new(),
    );
    work.close();
    assert!(work.settled(Moment::from_nanos(0))?);
    work.finish(Moment::from_nanos(0))?;
    Ok(())
}

/// Losing an unstarted wait never reports that a commit completed.
#[test]
fn abandoned_wait_reports_failure_without_completion() {
    let (fence, wait) = CompositionFence::fixture();
    drop(wait);
    assert!(fence.failed());
    assert!(!fence.completed());
}
