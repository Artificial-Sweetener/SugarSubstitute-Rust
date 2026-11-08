// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run media acquisition and later requests through the same real finite execution owner.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Condvar,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
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
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("Media wake poisoned")),
            )
        })? += 1;
        self.1.notify_all();
        Ok(())
    }
}
pub(super) struct Fixture {
    client: ExecutionClient,
    scope: Scope,
    clock: Arc<TestClock>,
    wake: Arc<Wake>,
    next: u64,
    _owner: ExecutionOwner,
}
impl Fixture {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let clock = Arc::new(TestClock::default());
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { client, scope, clock, wake, next: 0, _owner: owner })
    }
    pub(super) fn expire_acquisition_context(&self) {
        self.clock.0.store(2_000_000_000, Ordering::SeqCst);
    }
    pub(super) fn run<T: Send + 'static>(
        &mut self,
        deadline: u64,
        work: impl FnOnce(JobContext) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, Box<dyn Error>> {
        self.next += 1;
        let mut job = self.client.submit_blocking(
            &self.scope,
            JobSpec {
                id: JobId::new(self.next)?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: Some(Moment::from_nanos(deadline)),
                reservation_bytes: 2 * 1024 * 1024,
                replacement: None,
            },
            work,
        )?;
        loop {
            let revision = *self.wake.0.lock().map_err(|_| "Media wake poisoned")?;
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                return match completion.outcome {
                    JobOutcome::Success(value) => Ok(value),
                    JobOutcome::Failure(cause) => Err(cause.into()),
                    outcome => Err(format!("Media worker stopped: {:?}", outcome.kind()).into()),
                };
            }
            let (guard, timeout) = self
                .wake
                .1
                .wait_timeout_while(
                    self.wake.0.lock().map_err(|_| "Media wake poisoned")?,
                    Duration::from_secs(20),
                    |current| *current == revision,
                )
                .map_err(|_| "Media wake poisoned")?;
            if timeout.timed_out() && *guard == revision {
                return Err("Media worker did not terminate".into());
            }
        }
    }
}
