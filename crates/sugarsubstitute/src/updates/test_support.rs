// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared real-executor observation and deterministic clock for updater lifetimes.
use std::{
    error::Error,
    io,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, Moment, Operation, WaitFuture,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
pub(crate) struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut revision = self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("Update test wake poisoned")),
            )
        })?;
        *revision += 1;
        self.1.notify_all();
        Ok(())
    }
}
#[derive(Default)]
pub(crate) struct Manual(pub(crate) AtomicU64);
impl Clock for Manual {
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
}
/// Wait only on actual adapter notifications, with a bounded external observer cutoff.
pub(crate) fn observe(wake: &Wake, mut step: impl FnMut() -> Result<bool>) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let revision = *wake.0.lock().map_err(|_| "Update wake poisoned")?;
        if step()? {
            return Ok(());
        }
        let remaining =
            until.checked_duration_since(Instant::now()).ok_or("Update observer expired")?;
        let (guard, timeout) = wake
            .1
            .wait_timeout_while(
                wake.0.lock().map_err(|_| "Update wake poisoned")?,
                remaining,
                |current| *current == revision,
            )
            .map_err(|_| "Update wake poisoned")?;
        drop(guard);
        if timeout.timed_out() {
            if step()? {
                return Ok(());
            }
            return Err("Update observer expired".into());
        }
    }
}
