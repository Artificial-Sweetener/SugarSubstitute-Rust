// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move execution time only when a test advances it, so no deadline depends on machine speed.

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    task::{Context, Poll, Waker},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{Clock, Moment, TimeError, WaitFuture};

#[derive(Default)]
struct State {
    nanos: u64,
    next_sleeper: u64,
    sleepers: HashMap<u64, (Moment, Waker)>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    registered: Condvar,
}

impl Shared {
    fn locked(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One deterministic execution clock whose time moves only through [`ManualClock::advance`].
///
/// A deadline therefore expires exactly when a test chooses to reach it. A clock that is never
/// advanced never expires a deadline or completes a wait, which lets real native work take as long
/// as it needs under any machine load. Timer waits are ordinary futures polled by the execution
/// owner; the clock creates no thread, timer or scheduling authority of its own.
pub struct ManualClock {
    shared: Arc<Shared>,
}

impl ManualClock {
    /// Create a clock whose time starts at the zero moment of its epoch.
    pub fn new() -> Arc<Self> {
        Self::starting_at(Moment::from_nanos(0))
    }

    /// Create a clock whose time starts at `start`.
    pub fn starting_at(start: Moment) -> Arc<Self> {
        let shared = Shared::default();
        shared.locked().nanos = start.as_nanos();
        Arc::new(Self { shared: Arc::new(shared) })
    }

    /// Move time forward by `elapsed` and complete every wait whose deadline has been reached.
    ///
    /// # Errors
    /// Returns `TimeError::Overflow` when the elapsed interval or the resulting moment cannot be
    /// represented; time is left unchanged.
    pub fn advance(&self, elapsed: Duration) -> Result<(), TimeError> {
        let target = self.now().checked_add(elapsed)?;
        self.advance_to(target)
    }

    /// Move time forward to `moment` and complete every wait whose deadline has been reached.
    ///
    /// # Errors
    /// Returns `TimeError::Regression` when `moment` precedes the current time; time is left
    /// unchanged. Advancing to the current moment is accepted and completes nothing new.
    pub fn advance_to(&self, moment: Moment) -> Result<(), TimeError> {
        let due = {
            let mut state = self.shared.locked();
            if moment.as_nanos() < state.nanos {
                return Err(TimeError::Regression);
            }
            state.nanos = moment.as_nanos();
            let now = Moment::from_nanos(state.nanos);
            let due: Vec<u64> = state
                .sleepers
                .iter()
                .filter(|(_, (deadline, _))| *deadline <= now)
                .map(|(&id, _)| id)
                .collect();
            due.into_iter().filter_map(|id| state.sleepers.remove(&id)).collect::<Vec<_>>()
        };
        for (_, waker) in due {
            waker.wake();
        }
        Ok(())
    }

    /// Count the waits currently parked on this clock.
    pub fn sleepers(&self) -> usize {
        self.shared.locked().sleepers.len()
    }

    /// Block until a wait for exactly `deadline` is parked on this clock.
    ///
    /// The execution owner parks a wait only after polling the future it was handed, so this is the
    /// event that proves a timer-driven job has reached its wait. It has no wall-clock bound: the
    /// wait ends when the owner parks the sleeper, however long that takes.
    pub fn wait_for_sleeper(&self, deadline: Moment) {
        drop(
            self.shared
                .registered
                .wait_while(self.shared.locked(), |state| {
                    !state.sleepers.values().any(|(parked, _)| *parked == deadline)
                })
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.shared.locked().nanos)
    }

    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        let id = {
            let mut state = self.shared.locked();
            state.next_sleeper = state.next_sleeper.wrapping_add(1);
            state.next_sleeper
        };
        Box::pin(Sleep { shared: self.shared.clone(), deadline, id })
    }
}

/// Own one parked registration, released when the wait completes or is dropped.
struct Sleep {
    shared: Arc<Shared>,
    deadline: Moment,
    id: u64,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let mut state = self.shared.locked();
        if Moment::from_nanos(state.nanos) >= self.deadline {
            state.sleepers.remove(&self.id);
            return Poll::Ready(());
        }
        state.sleepers.insert(self.id, (self.deadline, context.waker().clone()));
        drop(state);
        self.shared.registered.notify_all();
        Poll::Pending
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        self.shared.locked().sleepers.remove(&self.id);
    }
}

#[cfg(test)]
mod tests;
