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
/// Construct the actual transcript source; no alternate snapshot constructor exists.
pub(super) fn transcript() -> TestResult<sugarsubstitute_application::transcript::TranscriptStore> {
    use sugarsubstitute_contracts::{lifecycle::*, transcript::*};
    let resource = ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let mut store = sugarsubstitute_application::transcript::TranscriptStore::new(resource);
    store.apply(&OutputBatch::new(
        resource,
        OutputStream::Stdout,
        StreamSequence::new(1)?,
        "original 日本語😀\n".as_bytes(),
        None,
        false,
    )?)?;
    Ok(store)
}
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
            if !self.wait_after(observed)? {
                return job
                    .try_take_completion()
                    .ok_or_else(|| "Preparation did not complete at the observed boundary".into());
            }
        }
    }
    /// Observe a real terminal result while leaving its typed payload unacknowledged.
    pub(super) fn terminal<T, E>(&self, job: &OwnedJob<T, E>) -> TestResult {
        loop {
            let observed = *self.wake.count.lock().map_err(|error| error.to_string())?;
            if matches!(job.status().phase, JobPhase::Terminal(_)) {
                return Ok(());
            }
            self.client.drain_completions(NonZeroUsize::MIN)?;
            if matches!(job.status().phase, JobPhase::Terminal(_)) {
                return Ok(());
            }
            if !self.wait_after(observed)? {
                return if matches!(job.status().phase, JobPhase::Terminal(_)) {
                    Ok(())
                } else {
                    Err("Preparation did not complete at the observed boundary".into())
                };
            }
        }
    }
    fn wait_after(&self, observed: u64) -> TestResult<bool> {
        let (count, timeout) = self
            .wake
            .changed
            .wait_timeout_while(
                self.wake.count.lock().map_err(|error| error.to_string())?,
                Duration::from_secs(20),
                |count| *count == observed,
            )
            .map_err(|error| error.to_string())?;
        Ok(!timeout.timed_out() || *count != observed)
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
