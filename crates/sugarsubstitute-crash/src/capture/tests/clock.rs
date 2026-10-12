// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inject a provider callback at the selected actual worker observation.
use super::super::lock;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};

pub(super) type ClockCallback = Box<dyn FnOnce() + Send>;
struct ObservationCallback {
    observer: u32,
    action: ClockCallback,
}
#[derive(Default)]
pub(super) struct ManualClock(pub(super) AtomicU64, Mutex<Option<ObservationCallback>>);
impl ManualClock {
    /// Select the actual currently admitted worker's next clock observation.
    pub(super) fn on_next_observation(&self, action: ClockCallback) {
        *lock(&self.1) =
            Some(ObservationCallback { observer: crate::native::current_clock_observer(), action });
    }
}
impl Clock for ManualClock {
    fn now(&self) -> Moment {
        let callback = {
            let mut stored = lock(&self.1);
            if stored.as_ref().is_some_and(|callback| {
                callback.observer == crate::native::current_clock_observer()
            }) {
                stored.take()
            } else {
                None
            }
        };
        if let Some(callback) = callback {
            (callback.action)();
        }
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
