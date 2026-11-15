// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Virtual time for the coordinator peer: its logical clock and its native wake instants.
use super::*;
use std::{
    cell::Cell,
    sync::atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};

/// Virtual time starts here on both peers' clocks, as the window's close fixture starts.
pub(super) const BASE: Duration = Duration::from_secs(1_000);

thread_local! {
    /// Virtual time on this coordinator thread, for the runner's native wake instants.
    pub(super) static NATIVE_ELAPSED: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

/// The runner's native instant: the frozen test anchor moved by virtual time.
pub(super) fn native_now() -> Instant {
    sugarsubstitute_execution::qualification_time::anchor() + BASE + NATIVE_ELAPSED.get()
}

/// The coordinator's logical clock, moved only by the pair. Its timers fire when virtual time
/// reaches them, and the earliest pending one is a deadline the pair may advance to.
#[derive(Default)]
pub(super) struct VirtualClock(Arc<ClockState>);
#[derive(Default)]
struct ClockState {
    elapsed: AtomicU64,
    next_timer: AtomicU64,
    timers: std::sync::Mutex<Vec<(u64, Moment, std::task::Waker)>>,
}
impl ClockState {
    fn now(&self) -> Moment {
        let base = u64::try_from(BASE.as_nanos()).unwrap_or(u64::MAX);
        Moment::from_nanos(base.saturating_add(self.elapsed.load(Ordering::SeqCst)))
    }
    fn timers(&self) -> std::sync::MutexGuard<'_, Vec<(u64, Moment, std::task::Waker)>> {
        self.timers.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
impl VirtualClock {
    pub(super) fn set(&self, elapsed: Duration) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.0.elapsed.store(nanos, Ordering::SeqCst);
        let now = self.0.now();
        let due: Vec<_> = {
            let mut timers = self.0.timers();
            let (due, waiting) = timers.drain(..).partition(|(_, at, _)| *at <= now);
            *timers = waiting;
            due
        };
        for (_, _, waker) in due {
            waker.wake();
        }
    }
    /// The earliest timer an executor job waits on.
    pub(super) fn next_timer(&self) -> Option<Moment> {
        self.0.timers().iter().map(|(_, at, _)| *at).min()
    }
}
/// One executor wait on the virtual clock; dropping it withdraws the wait.
struct Timer {
    state: Arc<ClockState>,
    id: u64,
    deadline: Moment,
}
impl std::future::Future for Timer {
    type Output = ();
    fn poll(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        if self.state.now() >= self.deadline {
            return std::task::Poll::Ready(());
        }
        let mut timers = self.state.timers();
        timers.retain(|(id, ..)| *id != self.id);
        timers.push((self.id, self.deadline, context.waker().clone()));
        std::task::Poll::Pending
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.state.timers().retain(|(id, ..)| *id != self.id);
    }
}
impl Clock for VirtualClock {
    fn now(&self) -> Moment {
        self.0.now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        let id = self.0.next_timer.fetch_add(1, Ordering::SeqCst);
        Box::pin(Timer { state: self.0.clone(), id, deadline })
    }
}

/// Virtual elapsed time of a coordinator moment.
pub(super) fn elapsed_of(moment: Moment) -> Option<Duration> {
    let base = Moment::from_nanos(u64::try_from(BASE.as_nanos()).ok()?);
    moment.checked_duration_since(base).ok()
}
