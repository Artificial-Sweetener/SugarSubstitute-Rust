// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit actual source/main/cleanup workers without constructing execution capabilities.
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, clock_fixture::ManualClock};
pub(super) type TestError = Box<dyn Error + Send + Sync>;
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(super) struct Harness {
    owner: Option<ExecutionOwner>,
    pub(super) client: ExecutionClient,
    pub(super) source: Scope,
    pub(super) main: Scope,
    pub(super) cleanup: Scope,
    wake: Arc<Wake>,
    clock: Arc<ManualClock>,
    next: u64,
}
impl Harness {
    pub(super) fn new() -> Result<Self, TestError> {
        let clock = ManualClock::new();
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let source =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let main = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let cleanup =
            client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
        Ok(Self { owner: Some(owner), client, source, main, cleanup, wake, clock, next: 0 })
    }
    pub(super) fn run<T: Send + 'static>(
        &mut self,
        which: u8,
        bytes: u64,
        call: impl FnOnce(JobContext) -> Result<T, TestError> + Send + 'static,
    ) -> Result<T, TestError> {
        self.next += 1;
        let scope = match which {
            1 => &self.source,
            2 => &self.main,
            _ => &self.cleanup,
        };
        let until =
            self.clock.now().checked_add(Duration::from_secs(if which == 3 { 10 } else { 60 }))?;
        let mut job = self.client.submit_blocking(
            scope,
            JobSpec {
                id: JobId::new(self.next)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(until),
                reservation_bytes: bytes,
                replacement: None,
            },
            call,
        )?;
        loop {
            let old = *self.wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            self.client.drain_completions(NonZeroUsize::MIN)?;
            if let Some(done) = job.try_take_completion() {
                return match done.outcome {
                    JobOutcome::Success(value) => Ok(value),
                    JobOutcome::Failure(cause) => Err(cause),
                    other => Err(format!("real worker outcome {:?}", other.kind()).into()),
                };
            }
            drop(
                self.wake
                    .changed
                    .wait_while(
                        self.wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                        |count| *count == old,
                    )
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }
    pub(super) fn join(&mut self) {
        drop(self.owner.take());
    }
}
