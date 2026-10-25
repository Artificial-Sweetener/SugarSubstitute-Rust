// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real execution completion using notifications and a deterministic application clock.
use super::*;
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Instant,
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, Moment, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;

#[derive(Default)]
pub(super) struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut value = self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("Render observer poisoned")),
            )
        })?;
        *value += 1;
        self.1.notify_all();
        Ok(())
    }
}
impl Wake {
    /// Synchronization time bounds failures; it never advances installer/application time.
    pub(super) fn observe<T>(&self, mut poll: impl FnMut() -> Result<Option<T>>) -> Result<T> {
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            let revision = *self.0.lock().map_err(|_| "Render observer poisoned")?;
            if let Some(result) = poll()? {
                return Ok(result);
            }
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("Render observer expired")?;
            let (guard, timed) = self
                .1
                .wait_timeout_while(
                    self.0.lock().map_err(|_| "Render observer poisoned")?,
                    remaining,
                    |current| *current == revision,
                )
                .map_err(|_| "Render observer poisoned")?;
            drop(guard);
            if timed.timed_out() {
                return poll()?.ok_or_else(|| "Render observer expired".into());
            }
        }
    }
}
struct Frozen;
impl Clock for Frozen {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

pub(super) struct Fixture {
    pub(super) owner: ExecutionOwner,
    pub(super) wake: Arc<Wake>,
    pub(super) driver: Driver,
    pub(super) flow: SetupFlow,
    pub(super) view: View,
}
impl Fixture {
    pub(super) fn new() -> Result<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(Frozen), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let driver = Driver::new(
            client,
            scope,
            JobSequence::new(),
            SurfaceId::new(1)?,
            FrameShape::new(720, 560, 1.0)?,
        )?;
        let view = View {
            surface: PaintSpec {
                width: 720,
                height: 560,
                scale: 1.0,
                theme: sugarsubstitute_contracts::configuration::Theme::Dark,
            },
            scroll: 0.0,
            now: Duration::ZERO,
            interaction: Interaction::default(),
            text_revisions: [0; 5],
        };
        Ok(Self { owner, wake, driver, flow: SetupFlow::new("E:\\Native".into())?, view })
    }
    pub(super) fn poll(&mut self) -> Result<Option<Prepared>> {
        let batch = self
            .owner
            .client()
            .drain_completions(std::num::NonZeroUsize::MIN.saturating_add(15))?;
        self.driver.advance(&self.flow, self.view, &batch)
    }
    pub(super) fn frame(&mut self) -> Result<Prepared> {
        self.wake.clone().observe(|| self.poll())
    }
    pub(super) fn terminal(&self) -> Result {
        self.wake.observe(|| {
            let active = self.driver.active.as_ref().ok_or("Active render absent")?;
            Ok(matches!(
                active.job.status().phase,
                sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
            )
            .then_some(()))
        })
    }
    pub(super) fn finish(&mut self) -> Result {
        self.driver.close()?;
        self.wake.clone().observe(|| {
            assert!(self.poll()?.is_none());
            Ok(self.driver.settled().then_some(()))
        })?;
        self.driver.finish()?;
        self.owner.request_shutdown(Moment::from_nanos(5_000_000_000));
        self.wake.clone().observe(|| {
            assert!(self.poll()?.is_none());
            Ok(self.owner.poll_shutdown()?.settled.then_some(()))
        })?;
        self.owner.finish_shutdown()?;
        Ok(())
    }
}
