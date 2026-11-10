// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run media acquisition and later requests through the same real finite execution owner.
use super::*;
use crate::completion_wake::Wake;
use std::sync::atomic::{AtomicU64, Ordering};
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
        let completion = self.wake.completion(&self.client, &mut job)?;
        match completion.outcome {
            JobOutcome::Success(value) => Ok(value),
            JobOutcome::Failure(cause) => Err(cause.into()),
            outcome => Err(format!("Media worker stopped: {:?}", outcome.kind()).into()),
        }
    }
}
