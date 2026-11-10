// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive actual source and HTTP owner jobs through one deterministic execution boundary.
use super::*;
use std::sync::{
    Condvar,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobPhase, JobSpec,
    Moment, Operation, Scope, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::installed_frontend::{
    FRONTEND_SOURCE_BYTES, FRONTEND_SOURCE_CLOSE_BYTES, InstalledFrontendOwner, boundary_fixture,
};
#[derive(Default)]
pub(super) struct TestClock(pub(super) AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
pub(super) struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.revision) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    pub(super) fn until<T>(
        &self,
        client: &ExecutionClient,
        mut ready: impl FnMut() -> Result<Option<T>, Box<dyn std::error::Error>>,
    ) -> Result<T, Box<dyn std::error::Error>> {
        loop {
            let revision = *lock(&self.revision);
            client.drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid drain")?)?;
            if let Some(value) = ready()? {
                return Ok(value);
            }
            drop(
                self.changed
                    .wait_while(lock(&self.revision), |current| *current == revision)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }
}
pub(super) struct Harness {
    _release: ReleaseOnDrop,
    pub(super) owner: ReadinessProbeOwner,
    source: InstalledFrontendOwner,
    executor: ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) attempt: Scope,
    pub(super) cleanup: Scope,
    pub(super) clock: Arc<TestClock>,
    pub(super) wake: Arc<Wake>,
    pub(super) boundary: Arc<super::boundary::Boundary>,
    next: u64,
}
struct ReleaseOnDrop(Arc<super::boundary::Boundary>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
impl Harness {
    pub(super) fn new(
        fail_slots: usize,
        late: bool,
        gate: bool,
        fail_close: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::default());
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = executor.client();
        let source_scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let attempt = client.create_scope(
            ScopeSpec { id: ScopeId::new(2)?, deadline: Some(Moment::from_nanos(500)) },
            None,
        )?;
        let cleanup =
            client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
        let mut source = boundary_fixture::owner()?;
        source.submit_inspection(
            &client,
            &source_scope,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::Readiness,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(10000)),
                reservation_bytes: FRONTEND_SOURCE_BYTES,
                replacement: None,
            },
        )?;
        let lease = wake.until(&client, || Ok(source.take_inspection()?))?;
        let boundary = Arc::new(super::boundary::Boundary::new(
            clock.clone(),
            fail_slots,
            late,
            gate,
            fail_close,
        ));
        let mut owner = ReadinessProbeOwner::new(ReadinessProbeInput::attached(lease, 8188)?);
        owner.port = boundary.clone();
        Ok(Self {
            _release: ReleaseOnDrop(boundary.clone()),
            owner,
            source,
            executor,
            client,
            attempt,
            cleanup,
            clock,
            wake,
            boundary,
            next: 2,
        })
    }
    pub(super) fn spec(&mut self, deadline: u64) -> Result<JobSpec, Box<dyn std::error::Error>> {
        let id = JobId::new(self.next)?;
        self.next += 1;
        Ok(JobSpec {
            id,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(deadline)),
            reservation_bytes: READINESS_TRANSPORT_BYTES,
            replacement: None,
        })
    }
    pub(super) fn submit(&mut self) -> TestResult {
        let spec = self.spec(1000)?;
        self.owner.submit_attempt(&self.client, &self.attempt, spec)?;
        Ok(())
    }
    pub(super) fn terminal(&self) -> TestResult {
        self.wake.until(&self.client, || {
            Ok(self
                .owner
                .pending
                .as_ref()
                .is_some_and(|job| matches!(job.status().phase, JobPhase::Terminal(_)))
                .then_some(()))
        })
    }
    pub(super) fn cleanup(
        &mut self,
    ) -> Result<
        sugarsubstitute_contracts::execution::JobCompletion<(), ReadinessTransportError>,
        Box<dyn std::error::Error>,
    > {
        let spec = self.spec(5000)?;
        let mut job = self.owner.submit_close(&self.client, &self.cleanup, spec)?;
        self.wake.until(&self.client, || Ok(job.try_take_completion()))
    }
    pub(super) fn retained(&self) -> (usize, usize, usize, u64, u64) {
        let guard = lock(&self.owner.shared.transaction);
        guard.as_ref().map_or((0, 0, 0, 0, 0), |state| {
            (
                state.value.fixture_slots[0],
                state.value.fixture_slots[1],
                state.value.fixture_slots[2],
                state._claim.bytes(),
                state.value.deadline.as_nanos(),
            )
        })
    }
    pub(super) fn finish(mut self) -> TestResult {
        if self.owner.pending.is_some() {
            return Err("Attempt completion was not consumed".into());
        }
        if lock(&self.owner.shared.transaction).is_some() {
            let completion = self.cleanup()?;
            if !matches!(completion.outcome, JobOutcome::Success(())) {
                return Err("Cleanup failed".into());
            }
        }
        let mut source_close = self.source.submit_close(
            &self.client,
            &self.cleanup,
            JobSpec {
                id: JobId::new(self.next)?,
                operation: Operation::Readiness,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(20000)),
                reservation_bytes: FRONTEND_SOURCE_CLOSE_BYTES,
                replacement: None,
            },
        )?;
        let completion =
            self.wake.until(&self.client, || Ok(source_close.try_take_completion()))?;
        if !matches!(completion.outcome, JobOutcome::Success(())) {
            return Err("Source cleanup failed".into());
        }
        drop(completion);
        drop(self.owner);
        drop(self.source);
        drop(self.cleanup);
        drop(self.attempt);
        drop(self.client);
        drop(self.executor);
        Ok(())
    }
}
