// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Inject clocks and synchronize controlled external work through production ports.

use crate::{runtime::ExecutionOwner, scope::lock};
use std::{
    collections::HashMap,
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context, Poll, Waker},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(crate) type ClockCallback = Box<dyn FnOnce() + Send>;
struct ObservationCallback {
    thread: std::thread::ThreadId,
    action: ClockCallback,
}
#[derive(Default)]
pub(crate) struct ManualClock {
    observation_callback: Mutex<Option<ObservationCallback>>,
    current: Mutex<MomentState>,
    next_waiter: AtomicU64,
    pub(crate) sleep_calls: AtomicU64,
    pub(crate) expired_sleep_calls: AtomicU64,
    registered: Condvar,
}
#[derive(Default)]
struct MomentState {
    nanos: u64,
    waiters: HashMap<u64, (Moment, Waker)>,
}

impl ManualClock {
    pub(crate) fn on_next_current_thread_observation(&self, action: ClockCallback) {
        *lock(&self.observation_callback) =
            Some(ObservationCallback { thread: std::thread::current().id(), action });
    }
    fn observe_callback(&self) {
        let callback = {
            let mut stored = lock(&self.observation_callback);
            if stored
                .as_ref()
                .is_some_and(|callback| callback.thread == std::thread::current().id())
            {
                stored.take()
            } else {
                None
            }
        };
        if let Some(callback) = callback {
            (callback.action)();
        }
    }
    pub(crate) fn wait_until_registered(&self, deadline: Moment) -> TestResult {
        let (state, timeout) = self
            .registered
            .wait_timeout_while(lock(&self.current), Duration::from_secs(10), |state| {
                !state.waiters.values().any(|(registered, _)| *registered == deadline)
            })
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if timeout.timed_out()
            && !state.waiters.values().any(|(registered, _)| *registered == deadline)
        {
            return Err(std::io::Error::other("Owned timer did not register").into());
        }
        Ok(())
    }
    pub(crate) fn advance(&self, nanos: u64) {
        let wakers = {
            let mut state = lock(&self.current);
            state.nanos += nanos;
            let now = Moment::from_nanos(state.nanos);
            state
                .waiters
                .extract_if(|_, (deadline, _)| *deadline <= now)
                .map(|(_, (_, waker))| waker)
                .collect::<Vec<_>>()
        };
        for waker in wakers {
            waker.wake();
        }
    }
}

struct ManualWait {
    clock: Arc<ManualClock>,
    deadline: Moment,
    id: u64,
}
impl Future for ManualWait {
    type Output = ();
    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let mut state = lock(&self.clock.current);
        if Moment::from_nanos(state.nanos) >= self.deadline {
            state.waiters.remove(&self.id);
            Poll::Ready(())
        } else {
            state.waiters.insert(self.id, (self.deadline, context.waker().clone()));
            self.clock.registered.notify_all();
            Poll::Pending
        }
    }
}
impl Drop for ManualWait {
    fn drop(&mut self) {
        lock(&self.clock.current).waiters.remove(&self.id);
    }
}

pub(crate) struct ClockPort(pub(crate) Arc<ManualClock>);
impl Clock for ClockPort {
    fn now(&self) -> Moment {
        self.0.observe_callback();
        Moment::from_nanos(lock(&self.0.current).nanos)
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        self.0.sleep_calls.fetch_add(1, Ordering::Relaxed);
        if deadline <= self.now() {
            self.0.expired_sleep_calls.fetch_add(1, Ordering::Relaxed);
        }
        Box::pin(ManualWait {
            clock: self.0.clone(),
            deadline,
            id: self.0.next_waiter.fetch_add(1, Ordering::Relaxed),
        })
    }
}

#[derive(Default)]
pub(crate) struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
    pub(crate) fail: AtomicBool,
}
impl Wake {
    pub(crate) fn count(&self) -> u64 {
        *lock(&self.count)
    }
    pub(crate) fn wait_after(&self, observed: u64) -> TestResult {
        let (count, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.count), Duration::from_secs(10), |count| {
                *count == observed
            })
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if timeout.timed_out() && *count == observed {
            return Err(std::io::Error::other("Controlled completion wake did not arrive").into());
        }
        Ok(())
    }
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.count) += 1;
        self.changed.notify_all();
        if self.fail.load(Ordering::Acquire) {
            Err(ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Owned wake fixture failed")),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<(bool, Vec<Waker>)>,
    changed: Condvar,
}
impl Gate {
    pub(crate) fn open(&self) {
        let wakers = {
            let mut state = lock(&self.state);
            state.0 = true;
            std::mem::take(&mut state.1)
        };
        self.changed.notify_all();
        for waker in wakers {
            waker.wake();
        }
    }
    pub(crate) fn wait(&self) -> TestResult {
        let (state, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.state), Duration::from_secs(10), |state| !state.0)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if timeout.timed_out() && !state.0 {
            return Err(std::io::Error::other(
                "Controlled work did not reach synchronization point",
            )
            .into());
        }
        Ok(())
    }
    pub(crate) fn future(self: &Arc<Self>) -> WaitFuture {
        let gate = self.clone();
        Box::pin(std::future::poll_fn(move |context| {
            let mut state = lock(&gate.state);
            if state.0 {
                Poll::Ready(())
            } else {
                if !state.1.iter().any(|waker| waker.will_wake(context.waker())) {
                    state.1.push(context.waker().clone());
                }
                Poll::Pending
            }
        }))
    }
}

/// Keep one CPU lane inside its step until the test opens the barrier.
pub(crate) struct Held {
    pub(crate) entered: Arc<Gate>,
    pub(crate) release: Arc<Gate>,
}
impl CpuTask<(), String> for Held {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, String> {
        self.entered.open();
        self.release.wait().map_err(|cause| cause.to_string())?;
        Ok(CpuStep::Complete(()))
    }
}

pub(crate) struct Rig {
    pub(crate) owner: ExecutionOwner,
    pub(crate) client: crate::runtime::ExecutionClient,
    pub(crate) clock: Arc<ManualClock>,
    pub(crate) wake: Arc<Wake>,
    pub(crate) scope: Scope,
}
impl Rig {
    pub(crate) fn new() -> TestResult<Self> {
        Self::with_workers(2)
    }
    pub(crate) fn with_workers(workers: usize) -> TestResult<Self> {
        let clock = Arc::new(ManualClock::default());
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::with_cpu_workers(
            Arc::new(ClockPort(clock.clone())),
            wake.clone(),
            workers,
        )?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { owner, client, clock, wake, scope })
    }
    pub(crate) fn completion<T, E>(
        &self,
        job: &mut OwnedJob<T, E>,
    ) -> TestResult<JobCompletion<T, E>> {
        loop {
            let observed = self.wake.count();
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            self.client.drain_completions(NonZeroUsize::MIN)?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            self.wake.wait_after(observed)?;
        }
    }
}

pub(crate) fn spec(id: u64, class: JobClass, bytes: u64) -> TestResult<JobSpec> {
    let operation = match class {
        JobClass::IoLifecycle => Operation::Startup,
        JobClass::CpuInteractive => Operation::FramePrepare,
        JobClass::CpuDecode => Operation::ImageDecode,
        _ => Operation::Infrastructure,
    };
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation,
        class,
        deadline: None,
        reservation_bytes: bytes,
        replacement: if class == JobClass::CpuInteractive {
            Some(ReplacementKey { surface: SurfaceId::new(1)?, revision: Generation::new(id)? })
        } else {
            None
        },
    })
}

/// Produce real admitted context with caller-selected labels, keeping its scope live.
pub(crate) fn admitted_context(
    rig: &Rig,
    scope_id: u64,
    job_id: u64,
) -> TestResult<(Scope, JobContext)> {
    let scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(scope_id)?, deadline: None }, None)?;
    let mut job = rig.client.submit_blocking(
        &scope,
        spec(job_id, JobClass::InfrastructureOrdinary, 4096)?,
        |context| Ok::<_, std::io::Error>(context.clone()),
    )?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(context) = completion.outcome else {
        return Err("Actual context factory did not succeed".into());
    };
    drop(completion.memory);
    Ok((scope, context))
}
