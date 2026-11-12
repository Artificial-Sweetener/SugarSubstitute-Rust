// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify owned context capabilities against deterministic manual boundary ports.

use super::*;
use crate::execution::{Generation, JobClass, MemoryRelease, Operation, ReplacementKey, SurfaceId};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

#[derive(Default)]
struct WaitState {
    value: AtomicU64,
    waker: Mutex<Option<Waker>>,
}

impl WaitState {
    fn advance(&self, value: u64) {
        self.value.store(value, Ordering::SeqCst);
        if let Some(waker) =
            self.waker.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take()
        {
            waker.wake();
        }
    }
}

struct ValueWait {
    state: Arc<WaitState>,
    threshold: u64,
}

impl Future for ValueWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.state.value.load(Ordering::SeqCst) >= self.threshold {
            Poll::Ready(())
        } else {
            *self.state.waker.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(context.waker().clone());
            Poll::Pending
        }
    }
}

struct ManualClock(Arc<WaitState>);
impl Clock for ManualClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.value.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        Box::pin(ValueWait { state: self.0.clone(), threshold: deadline.as_nanos() })
    }
}

struct ManualCancel(Arc<WaitState>);
impl CancelControl for ManualCancel {
    fn is_requested(&self) -> bool {
        self.0.value.load(Ordering::SeqCst) != 0
    }
    fn cancelled(&self) -> WaitFuture {
        Box::pin(ValueWait { state: self.0.clone(), threshold: 1 })
    }
}

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct ReleaseCount(Arc<AtomicU64>);
impl MemoryRelease for ReleaseCount {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}

#[test]
fn context_preserves_identity_deadline_and_owned_waits_without_scheduler_access()
-> Result<(), Box<dyn std::error::Error>> {
    let owner_id = ExecutionId::new(7)?;
    let scope_id = ScopeId::new(9)?;
    let spec = JobSpec {
        id: JobId::new(14)?,
        operation: Operation::FramePrepare,
        class: JobClass::CpuInteractive,
        deadline: Some(Moment::from_nanos(90)),
        reservation_bytes: 12,
        replacement: Some(ReplacementKey {
            surface: SurfaceId::new(3)?,
            revision: Generation::new(1)?,
        }),
    };
    let clock_state = Arc::new(WaitState::default());
    let cancel_state = Arc::new(WaitState::default());
    let released = Arc::new(AtomicU64::new(0));
    let context = JobContext::new(
        owner_id,
        scope_id,
        spec,
        Some(Moment::from_nanos(60)),
        Arc::new(ManualCancel(cancel_state.clone())),
        Arc::new(ManualClock(clock_state.clone())),
        AdmittedJobResources::new(
            MemoryLease::new(12, Box::new(ReleaseCount(released.clone()))),
            None,
        ),
    );
    let retained = context.clone();
    assert_eq!(retained.owner_id(), owner_id);
    assert_eq!(retained.scope_id(), scope_id);
    assert_eq!(retained.job_id(), spec.id);
    assert_eq!(retained.spec(), spec);
    assert_eq!(retained.deadline(), Some(Moment::from_nanos(60)));
    assert_eq!(retained.now(), Moment::from_nanos(0));
    assert!(!retained.cancellation_requested());
    let bounded = context.bounded_until(Moment::from_nanos(30));
    assert_eq!(bounded.deadline(), Some(Moment::from_nanos(30)));
    assert_eq!(bounded.owner_id(), owner_id);
    assert_eq!(bounded.scope_id(), scope_id);
    assert_eq!(bounded.job_id(), spec.id);
    assert_eq!(bounded.spec(), spec);
    assert_eq!(bounded.bounded_until(Moment::from_nanos(100)).deadline(), bounded.deadline());
    assert_eq!(context.bounded_until(Moment::from_nanos(100)).deadline(), context.deadline());
    let claim = bounded.memory().claim(10)?;
    assert!(context.memory().claim(3).is_err());
    drop(claim);

    let wakes = Arc::new(WakeCount::default());
    let waker = Waker::from(wakes.clone());
    let mut poll_context = Context::from_waker(&waker);
    let mut cancelled = context.cancelled();
    let mut deadline = context.sleep_until(Moment::from_nanos(60));
    assert_eq!(cancelled.as_mut().poll(&mut poll_context), Poll::Pending);
    assert_eq!(deadline.as_mut().poll(&mut poll_context), Poll::Pending);
    drop(context);
    let result_guard = retained.memory().clone();
    cancel_state.advance(1);
    assert!(retained.cancellation_requested());
    assert!(bounded.cancellation_requested());
    clock_state.advance(60);
    assert_eq!(retained.now(), Moment::from_nanos(60));
    assert_eq!(bounded.now(), Moment::from_nanos(60));
    drop(bounded);
    drop(retained);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 2);
    assert_eq!(cancelled.as_mut().poll(&mut poll_context), Poll::Ready(()));
    assert_eq!(deadline.as_mut().poll(&mut poll_context), Poll::Ready(()));
    drop(result_guard);
    assert_eq!(released.load(Ordering::SeqCst), 12);
    Ok(())
}

#[test]
fn command_lifetime_can_bound_an_operation_without_extending_that_bound()
-> Result<(), Box<dyn std::error::Error>> {
    let context = JobContext::new(
        ExecutionId::new(1)?,
        ScopeId::new(1)?,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: 12,
            replacement: None,
        },
        None,
        Arc::new(ManualCancel(Arc::new(WaitState::default()))),
        Arc::new(ManualClock(Arc::new(WaitState::default()))),
        AdmittedJobResources::new(
            MemoryLease::new(12, Box::new(ReleaseCount(Arc::new(AtomicU64::new(0))))),
            None,
        ),
    );
    let operation = context.bounded_until(Moment::from_nanos(30));
    assert_eq!(context.deadline(), None);
    assert_eq!(operation.deadline(), Some(Moment::from_nanos(30)));
    assert_eq!(operation.bounded_until(Moment::from_nanos(90)).deadline(), operation.deadline());
    assert_eq!(operation.bounded_until(Moment::from_nanos(0)).deadline(), Some(context.now()));
    Ok(())
}

/// Run fork-join items in reverse order on the caller, recording each call's group count.
#[derive(Default)]
struct ReverseJoin(std::sync::Mutex<Vec<usize>>);
impl ForkJoin for ReverseJoin {
    fn width(&self) -> usize {
        3
    }
    fn join<'a>(&self, items: Vec<ForkJoinItem<'a>>) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(items.len());
        for item in items.into_iter().rev() {
            item();
        }
    }
}

fn chunk_context(
    fork_join: Option<Arc<dyn ForkJoin>>,
) -> Result<JobContext, Box<dyn std::error::Error>> {
    let resources = AdmittedJobResources::new(
        MemoryLease::new(1, Box::new(ReleaseCount(Arc::new(AtomicU64::new(0))))),
        None,
    );
    Ok(JobContext::new(
        ExecutionId::new(1)?,
        ScopeId::new(1)?,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: 1,
            replacement: None,
        },
        None,
        Arc::new(ManualCancel(Arc::new(WaitState::default()))),
        Arc::new(ManualClock(Arc::new(WaitState::default()))),
        match fork_join {
            Some(fork_join) => resources.with_fork_join(fork_join),
            None => resources,
        },
    ))
}

#[test]
fn chunks_run_in_order_on_the_caller_without_fork_join() -> Result<(), Box<dyn std::error::Error>> {
    let context = chunk_context(None)?;
    let mut data = [0_usize; 7];
    let visited = std::sync::Mutex::new(Vec::new());
    context.try_for_each_chunk_mut(&mut data, 3, |index, chunk| {
        visited.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(index);
        chunk.fill(index + 1);
        Ok::<_, std::fmt::Error>(())
    })?;
    assert_eq!(data, [1, 1, 1, 2, 2, 2, 3]);
    assert_eq!(*visited.lock().unwrap_or_else(std::sync::PoisonError::into_inner), [0, 1, 2]);
    let failed =
        context.try_for_each_chunk_mut(
            &mut data,
            0,
            |index, _| {
                if index == 4 { Err(index) } else { Ok(()) }
            },
        );
    assert_eq!(failed, Err(4));
    Ok(())
}

#[test]
fn fork_join_groups_cover_every_chunk_once_and_report_the_earliest_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let join = Arc::new(ReverseJoin::default());
    let context = chunk_context(Some(join.clone()))?;
    let mut data = [0_usize; 23];
    context.try_for_each_chunk_mut(&mut data, 2, |index, chunk| {
        chunk.iter_mut().for_each(|value| *value += index + 1);
        Ok::<_, std::fmt::Error>(())
    })?;
    let expected: Vec<usize> = (0..23).map(|offset| offset / 2 + 1).collect();
    assert_eq!(data.to_vec(), expected);
    let failed = context.try_for_each_chunk_mut(&mut data, 2, |index, _| {
        if index == 2 || index == 9 { Err(index) } else { Ok(()) }
    });
    assert_eq!(failed, Err(2));
    let single = context.try_for_each_chunk_mut(&mut data[..2], 2, |_, _| Ok::<_, ()>(()));
    assert_eq!(single, Ok(()));
    assert_eq!(*join.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner), [3, 3]);
    Ok(())
}
