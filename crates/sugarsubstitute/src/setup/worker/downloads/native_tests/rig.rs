// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual owned jobs through the production execution boundary and bounded notifications.
use super::*;
use std::{num::NonZeroUsize, sync::Condvar, time::Instant};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, JobCompletion, ScopeId, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;

/// Operation timers remain controlled by the executor; wall time only guards a failed test wait.
struct ClockZero;
impl Clock for ClockZero {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Wake a test observer after an actual terminal outcome is available.
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut revision = self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("Native download observer poisoned")),
            )
        })?;
        *revision += 1;
        self.1.notify_all();
        Ok(())
    }
}
/// Real runtime, admission and shutdown; no fixture thread or executor is constructed.
pub(super) struct Rig {
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    jobs: JobSequence,
    wake: Arc<Wake>,
    _owner: ExecutionOwner,
}
impl Rig {
    /// Create one isolated execution scope for a native transport and its admitted storage service.
    pub(super) fn new() -> Result<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(ClockZero), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { client, scope, jobs: JobSequence::new(), wake, _owner: owner })
    }
    /// Allocate identities only on the test's owner thread, matching the host's job sequencing.
    pub(super) fn spec(&self, class: JobClass, bytes: u64) -> Result<JobSpec> {
        Ok(JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class,
            reservation_bytes: bytes,
            deadline: None,
            replacement: None,
        })
    }
    /// Observe terminal delivery without polling sleeps or manufacturing job completion.
    pub(super) fn completion<T, E>(
        &self,
        job: &mut OwnedJob<T, E>,
        phase: &'static str,
    ) -> Result<JobCompletion<T, E>> {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            let observed = *self.wake.0.lock().map_err(|_| "Native download observer poisoned")?;
            self.client.drain_completions(NonZeroUsize::MIN)?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            let revision = self.wake.0.lock().map_err(|_| "Native download observer poisoned")?;
            if *revision != observed {
                continue;
            }
            let remaining = until
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("Native download {phase} observation deadline elapsed"))?;
            let (guard, timed) = self
                .wake
                .1
                .wait_timeout_while(revision, remaining, |revision| *revision == observed)
                .map_err(|_| "Native download observer poisoned")?;
            drop(guard);
            if timed.timed_out() {
                self.client.drain_completions(NonZeroUsize::MIN)?;
                if let Some(completion) = job.try_take_completion() {
                    return Ok(completion);
                }
                job.cancel();
                return Err(format!(
                    "Native download {phase} did not settle; {} bytes remain admitted",
                    self.scope.snapshot().reserved_bytes
                )
                .into());
            }
        }
    }
    /// Run filesystem fixture effects on an admitted blocking worker, preserving real original claims.
    pub(super) fn storage<T: Send + 'static>(
        &self,
        phase: &'static str,
        bytes: u64,
        body: impl FnOnce(JobContext) -> io::Result<T> + Send + 'static,
    ) -> Result<T> {
        let mut specification = self.spec(JobClass::InfrastructureOrdinary, bytes)?;
        specification.deadline = Some(Moment::from_nanos(30_000_000_000));
        let mut job = self.client.submit_blocking(&self.scope, specification, body)?;
        match self.completion(&mut job, phase)?.outcome {
            JobOutcome::Success(value) => Ok(value),
            JobOutcome::Failure(cause) => {
                let mut chain = Vec::new();
                let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&cause);
                while let Some(error) = current {
                    chain.push(error.to_string());
                    current = error.source();
                }
                Err(format!("Native fixture storage failed: {}", chain.join(": ")).into())
            }
            outcome => Err(format!("Native fixture job failed: {:?}", outcome.kind()).into()),
        }
    }
}
