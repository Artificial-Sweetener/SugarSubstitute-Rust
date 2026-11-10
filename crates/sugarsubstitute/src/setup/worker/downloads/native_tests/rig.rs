// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual owned jobs through the production execution boundary and bounded notifications.
use super::*;
use crate::completion_wake::Wake;
use sugarsubstitute_contracts::execution::{Clock, JobCompletion, WaitFuture};
use sugarsubstitute_execution::ExecutionOwner;

/// Operation timers remain controlled by the executor and never expire.
struct ClockZero;
impl Clock for ClockZero {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
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
    /// Observe terminal delivery on the executor's own wakes, however long the work takes.
    pub(super) fn completion<T, E>(&self, job: &mut OwnedJob<T, E>) -> Result<JobCompletion<T, E>> {
        self.wake.completion(&self.client, job)
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
        match self.completion(&mut job)?.outcome {
            JobOutcome::Success(value) => Ok(value),
            JobOutcome::Failure(cause) => {
                let mut chain = Vec::new();
                let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&cause);
                while let Some(error) = current {
                    chain.push(error.to_string());
                    current = error.source();
                }
                Err(format!("Native fixture {phase} storage failed: {}", chain.join(": ")).into())
            }
            outcome => {
                Err(format!("Native fixture {phase} job failed: {:?}", outcome.kind()).into())
            }
        }
    }
}
