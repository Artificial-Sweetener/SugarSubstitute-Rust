// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expire qualification only at an observed native boundary, independent of wall-clock delays.
use std::{
    io,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{Clock, Moment, WaitFuture};

#[derive(Default)]
pub(crate) struct TestClock {
    now: AtomicU64,
    observation: OnceLock<Box<dyn Fn() -> bool + Send + Sync>>,
}
impl TestClock {
    /// Make the next operation check observe its original deadline.
    pub(crate) fn expire(&self) {
        self.now.store(2_000_000_000, Ordering::SeqCst);
    }
    /// Tie expiry to the isolated fixture's actual native connection.
    pub(crate) fn observe_checks(
        &self,
        observe: impl Fn() -> bool + Send + Sync + 'static,
    ) -> io::Result<()> {
        self.observation
            .set(Box::new(observe))
            .map_err(|_| io::Error::other("Clock observer already set"))
    }
}
impl Clock for TestClock {
    fn now(&self) -> Moment {
        if self.observation.get().is_some_and(|observe| observe()) {
            self.expire();
        }
        Moment::from_nanos(self.now.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
