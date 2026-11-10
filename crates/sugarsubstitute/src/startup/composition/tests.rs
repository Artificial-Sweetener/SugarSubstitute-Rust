// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise owned admission and retirement with a controlled compositor boundary.

use super::*;
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionError, Generation, JobCompletion, ScopeId, WaitFuture,
    },
    startup::LaunchId,
};
use sugarsubstitute_execution::ExecutionOwner;

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

#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn revision(&self) -> u64 {
        *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn wait_after(&self, revision: u64) -> Result<(), Box<dyn Error>> {
        let guard = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (guard, timeout) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |value| *value == revision)
            .map_err(|cause| io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *guard == revision {
            return Err("Composition completion did not arrive".into());
        }
        Ok(())
    }
    fn completion<T, E>(
        &self,
        client: &ExecutionClient,
        job: &mut OwnedJob<T, E>,
    ) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
        loop {
            let revision = self.revision();
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            self.wait_after(revision)?;
        }
    }

    fn settle(
        &self,
        client: &ExecutionClient,
        work: &mut CompositionWork,
        now: Moment,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            let revision = self.revision();
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if work.settled(now)? {
                return Ok(());
            }
            self.wait_after(revision)?;
        }
    }
}

fn identity(navigation: u64) -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: super::super::policy::tests::test_run()?,
        launch: LaunchId::new(1)?,
        navigation: Generation::new(navigation)?,
    })
}

/// A saturated reservation is retried with a new job identity, however long admission takes.
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
    assert_eq!(work.deadline(), None, "a wait has no cutoff of its own before shutdown");
    assert!(!fence.completed());
    // Two hours of injected time pass while admission stays saturated: nothing expires.
    work.advance(Moment::from_nanos(7_200_000_000_000))?;
    assert!(work.pending.is_some() && !fence.failed());
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
    // The retry at two hours consumed one identity; admission then used the next.
    assert_eq!(work.active.as_ref().ok_or("Wait was not admitted")?.job.spec().id.get(), 5);
    work.close();
    assert!(!work.settled(Moment::from_nanos(0))?);
    assert!(work.main_retired(identity(2)?).is_err());
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        work.advance(Moment::from_nanos(0))?;
        if work.active.is_none() {
            break;
        }
        wake.wait_after(revision)?;
    }
    assert!(fence.completed());
    work.main_retired(main)?;
    wake.settle(&client, &mut work, Moment::from_nanos(0))?;
    work.finish(Moment::from_nanos(0))?;
    owner.request_shutdown(Moment::from_nanos(10_000_000_000));
    let deadline =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Shutdown deadline overflow")?;
    while !owner.poll_shutdown()?.settled {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if Instant::now() >= deadline {
            return Err("Execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
    Ok(())
}

#[derive(Default)]
struct HoldState {
    entered: bool,
    released: bool,
}

#[derive(Default)]
struct Hold {
    state: Mutex<HoldState>,
    changed: Condvar,
}

impl Hold {
    fn run(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.entered = true;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    fn wait_for_entry(&self) -> Result<(), Box<dyn Error>> {
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.entered)
            .map_err(|cause| io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && !state.entered {
            return Err("Wait worker did not enter".into());
        }
        Ok(())
    }

    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.released = true;
        self.changed.notify_all();
    }
}

struct Release(Arc<Hold>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
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
    let hold = Arc::new(Hold::default());
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
            worker.run();
            input.wait(&context)
        },
    )?;
    work.active = Some(Active {
        fence: fence.clone(),
        job,
        deadline: Some(Moment::from_nanos(10)),
        expiry_reported: false,
    });
    hold.wait_for_entry()?;
    clock.0.store(10, Ordering::SeqCst);
    assert!(work.advance(Moment::from_nanos(10)).is_err());
    assert!(!fence.completed());
    assert!(!fence.failed());
    work.close();
    assert!(!work.settled(Moment::from_nanos(10))?);
    assert!(work.main_retired(main).is_err());
    release.0.release();
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        work.advance(Moment::from_nanos(10))?;
        if work.active.is_none() {
            break;
        }
        wake.wait_after(revision)?;
    }
    assert!(fence.completed());
    work.main_retired(main)?;
    wake.settle(&client, &mut work, Moment::from_nanos(10))?;
    work.finish(Moment::from_nanos(10))?;
    owner.request_shutdown(Moment::from_nanos(10_000_000_000));
    let deadline =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Shutdown deadline overflow")?;
    while !owner.poll_shutdown()?.settled {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if Instant::now() >= deadline {
            return Err("Execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
    Ok(())
}

/// A compositor that is slow to commit delays the wait; with no cutoff it is neither expired nor
/// reported as a failure.
#[test]
fn a_slow_compositor_wait_without_a_cutoff_completes_without_an_error() -> Result<(), Box<dyn Error>>
{
    const LATER: Moment = Moment::from_nanos(7_200_000_000_000);
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
    let hold = Arc::new(Hold::default());
    let release = Release(hold.clone());
    let worker = hold.clone();
    let spec = JobSpec {
        id: jobs.allocate()?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureLifecycle,
        deadline: None,
        reservation_bytes: 64 * 1024,
        replacement: None,
    };
    let job = client.submit_blocking_owned_input(
        work.scope.as_ref().ok_or("Scope absent")?,
        spec,
        &mut input,
        move |input, context| {
            worker.run();
            input.wait(&context)
        },
    )?;
    work.active =
        Some(Active { fence: fence.clone(), job, deadline: None, expiry_reported: false });
    hold.wait_for_entry()?;
    clock.0.store(LATER.as_nanos(), Ordering::SeqCst);
    work.advance(LATER)?;
    assert!(!fence.completed() && !fence.failed(), "a slow wait is not a failure");
    assert_eq!(work.deadline(), None);
    work.close();
    assert!(!work.settled(LATER)?);
    release.0.release();
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        work.advance(LATER)?;
        if work.active.is_none() {
            break;
        }
        wake.wait_after(revision)?;
    }
    assert!(fence.completed(), "the released wait completed without an error");
    work.main_retired(main)?;
    wake.settle(&client, &mut work, LATER)?;
    work.finish(LATER)?;
    owner.request_shutdown(Moment::from_nanos(LATER.as_nanos() + 10_000_000_000));
    let deadline =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Shutdown deadline overflow")?;
    while !owner.poll_shutdown()?.settled {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if Instant::now() >= deadline {
            return Err("Execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
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
    let hold = Arc::new(Hold::default());
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
            worker.run();
            Ok(())
        },
    )?;
    work.active = Some(Active {
        fence: fence.clone(),
        job,
        deadline: Some(Moment::from_nanos(30)),
        expiry_reported: false,
    });
    hold.wait_for_entry()?;
    assert!(fence.completed());

    work.main_retired(main)?;
    assert!(!work.register(replacement)?);

    clock.0.store(31, Ordering::SeqCst);
    release.0.release();
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        work.advance(Moment::from_nanos(31))?;
        if work.active.is_none() {
            break;
        }
        wake.wait_after(revision)?;
    }
    assert!(work.register(replacement)?);
    work.close();
    work.main_retired(replacement)?;
    wake.settle(&client, &mut work, Moment::from_nanos(31))?;
    work.finish(Moment::from_nanos(31))?;

    owner.request_shutdown(Moment::from_nanos(10_000_000_000));
    let deadline =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Shutdown deadline overflow")?;
    while !owner.poll_shutdown()?.settled {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if Instant::now() >= deadline {
            return Err("Execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
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
    work.pending = Some(Pending {
        input: Some(input),
        fence: fence.clone(),
        deadline: Some(Moment::from_nanos(1)),
    });
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
