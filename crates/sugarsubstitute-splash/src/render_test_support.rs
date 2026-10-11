// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise preparation through the actual central execution boundary with observed wakes.

use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _deadline: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut count = self.count.lock().map_err(|error| {
            ExecutionError::new(
                Operation::FramePrepare,
                Box::new(std::io::Error::other(error.to_string())),
            )
        })?;
        *count += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(super) struct Rig {
    // Owner drops after typed jobs/results in each test, joining actual workers.
    pub(super) owner: ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    wake: Arc<Wake>,
}
impl Rig {
    pub(super) fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { owner, client, scope, wake })
    }
    pub(super) fn completion<T, E>(
        &self,
        job: &mut OwnedJob<T, E>,
    ) -> TestResult<JobCompletion<T, E>> {
        loop {
            let observed = *self.wake.count.lock().map_err(|error| error.to_string())?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            self.client.drain_completions(NonZeroUsize::MIN)?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            let (count, timeout) = self
                .wake
                .changed
                .wait_timeout_while(
                    self.wake.count.lock().map_err(|error| error.to_string())?,
                    Duration::from_secs(20),
                    |count| *count == observed,
                )
                .map_err(|error| error.to_string())?;
            if timeout.timed_out() && *count == observed {
                return Err("Preparation did not complete at the observed boundary".into());
            }
        }
    }
}
pub(super) fn spec(id: u64, bytes: u64) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::FramePrepare,
        class: JobClass::CpuInteractive,
        deadline: None,
        reservation_bytes: bytes,
        replacement: Some(ReplacementKey {
            surface: SurfaceId::new(1)?,
            revision: Generation::new(id)?,
        }),
    })
}
pub(super) fn success<T, E>(completion: JobCompletion<T, E>) -> TestResult<T> {
    match completion.outcome {
        JobOutcome::Success(value) => Ok(value),
        _ => Err("Expected successful preparation task outcome".into()),
    }
}

/// Synchronize a finite controlled step without adding a thread or arbitrary sleep.
#[derive(Default)]
pub(super) struct Gate {
    open: Mutex<bool>,
    changed: Condvar,
}
impl Gate {
    pub(super) fn open(&self) -> TestResult {
        *self.open.lock().map_err(|error| error.to_string())? = true;
        self.changed.notify_all();
        Ok(())
    }
    pub(super) fn wait(&self) -> TestResult {
        let (open, timeout) = self
            .changed
            .wait_timeout_while(
                self.open.lock().map_err(|error| error.to_string())?,
                Duration::from_secs(20),
                |open| !*open,
            )
            .map_err(|error| error.to_string())?;
        if timeout.timed_out() && !*open {
            return Err("Controlled render boundary did not return".into());
        }
        Ok(())
    }
}
