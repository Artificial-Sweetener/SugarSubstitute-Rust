// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Control disposal of an actual injected timer without scheduling test work.

use crate::{
    runtime_test_support::{ClockPort, Gate, ManualClock},
    scope::lock,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};

pub(super) struct TimerBoundary {
    pub(super) clock: Arc<ManualClock>,
    pub(super) armed: AtomicBool,
    hold_first_poll: AtomicBool,
    pub(super) polling: Gate,
    pub(super) poll_release: Gate,
    pub(super) disposing: Gate,
    pub(super) release: Gate,
    pub(super) native_start_release: Gate,
    pub(super) native_thread: OnceLock<std::thread::ThreadId>,
    pub(super) native_clock_reads: AtomicUsize,
    pub(super) native_zero_observed: Gate,
    pub(super) failure: Mutex<Option<String>>,
}
impl TimerBoundary {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            clock: Arc::new(ManualClock::default()),
            armed: AtomicBool::new(false),
            hold_first_poll: AtomicBool::new(true),
            polling: Gate::default(),
            poll_release: Gate::default(),
            disposing: Gate::default(),
            release: Gate::default(),
            native_start_release: Gate::default(),
            native_thread: OnceLock::new(),
            native_clock_reads: AtomicUsize::new(0),
            native_zero_observed: Gate::default(),
            failure: Mutex::new(None),
        })
    }
}
pub(super) struct ControlledClock(pub(super) Arc<TimerBoundary>);
impl Clock for ControlledClock {
    fn now(&self) -> Moment {
        let now = ClockPort(self.0.clock.clone()).now();
        if self.0.native_thread.get() == Some(&std::thread::current().id())
            && now == Moment::from_nanos(0)
            && self.0.native_clock_reads.fetch_add(1, Ordering::AcqRel) == 2
        {
            // After start: progress reads time, park consumes the prior token,
            // then the next loop and progress read time. No Clock call blocks.
            self.0.native_zero_observed.open();
        }
        now
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        Box::pin(Timer {
            inner: ClockPort(self.0.clock.clone()).sleep_until(deadline),
            boundary: self.0.clone(),
        })
    }
}
struct Timer {
    inner: WaitFuture,
    boundary: Arc<TimerBoundary>,
}
impl Future for Timer {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let result = self.inner.as_mut().poll(context);
        if result.is_pending() && self.boundary.hold_first_poll.swap(false, Ordering::AcqRel) {
            // Keep this registered timer alive until the test arms its disposal
            // and publishes the notification that must win the actual select.
            self.boundary.polling.open();
            if let Err(error) = self.boundary.poll_release.wait() {
                *lock(&self.boundary.failure) = Some(error.to_string());
            }
        }
        result
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        if self.boundary.armed.swap(false, Ordering::AcqRel) {
            self.boundary.disposing.open();
            if let Err(error) = self.boundary.release.wait() {
                *lock(&self.boundary.failure) = Some(error.to_string());
            }
        }
    }
}
pub(super) struct ReleaseOnDrop(pub(super) Arc<TimerBoundary>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.poll_release.open();
        self.0.release.open();
        self.0.native_start_release.open();
    }
}
