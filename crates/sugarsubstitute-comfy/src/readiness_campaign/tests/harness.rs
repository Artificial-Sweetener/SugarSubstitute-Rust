// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive actual source/probe/cleanup lifetimes through one deterministic execution owner.
use super::*;
use crate::readiness_transport::{
    READINESS_TRANSPORT_BYTES,
    campaign_fixture::{Behavior, FixtureHttp},
};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionBatch, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId,
    JobSpec, Operation, Scope, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::installed_frontend::{
    FRONTEND_SOURCE_BYTES, InstalledFrontendLease, InstalledFrontendOwner, boundary_fixture,
};
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
struct TestClock(Arc<AtomicU64>);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
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
    fn until<T>(
        &self,
        client: &ExecutionClient,
        mut ready: impl FnMut(CompletionBatch) -> Result<Option<T>, Box<dyn std::error::Error>>,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let revision = *lock(&self.revision);
            let batch = client.drain_completions(NonZeroUsize::new(32).ok_or("Invalid drain")?)?;
            if let Some(value) = ready(batch)? {
                return Ok(value);
            }
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("Missing causal completion")?;
            let (_guard, status) = self
                .changed
                .wait_timeout_while(lock(&self.revision), remaining, |current| *current == revision)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if status.timed_out() {
                return Err("Missing causal completion wake".into());
            }
        }
    }
}
pub(super) struct Harness {
    fixtures: FixtureGuard,
    pub(super) campaign: ReadinessCampaign,
    source: InstalledFrontendOwner,
    pub(super) lease: InstalledFrontendLease,
    executor: ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) attempt: Scope,
    pub(super) cleanup: Scope,
    pub(super) clock: Arc<AtomicU64>,
    wake: Arc<Wake>,
    next_job: u64,
    saved: Option<CompletionBatch>,
}
impl Harness {
    pub(super) fn new() -> TestResult<Self> {
        let clock = Arc::new(AtomicU64::new(0));
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(Arc::new(TestClock(clock.clone())), wake.clone())?;
        let client = executor.client();
        let source_scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let attempt =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
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
                deadline: Some(Moment::from_nanos(60_000_000_000)),
                reservation_bytes: FRONTEND_SOURCE_BYTES,
                replacement: None,
            },
        )?;
        let lease = wake.until(&client, |_| Ok(source.take_inspection()?))?;
        let campaign = ReadinessCampaign::new(
            attached_target(8188)?,
            ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?,
        )?;
        Ok(Self {
            fixtures: FixtureGuard(Vec::new()),
            campaign,
            source,
            lease,
            executor,
            client,
            attempt,
            cleanup,
            clock,
            wake,
            next_job: 2,
            saved: None,
        })
    }
    pub(super) fn now(&self) -> Moment {
        Moment::from_nanos(self.clock.load(Ordering::Acquire))
    }
    pub(super) fn set_time(&self, nanos: u64) {
        self.clock.store(nanos, Ordering::Release);
    }
    pub(super) fn spec(&mut self, until: Moment) -> TestResult<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job += 1;
        Ok(JobSpec {
            id,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: READINESS_TRANSPORT_BYTES,
            replacement: None,
        })
    }
    pub(super) fn submit(
        &mut self,
        behavior: Behavior,
        gated: Option<&'static str>,
        fail_close: bool,
        finish_at: Option<Moment>,
    ) -> TestResult<Arc<FixtureHttp>> {
        let input = ReadinessProbeInput::attached(self.lease.clone(), 8188)?;
        let fixture = FixtureHttp::new(behavior, gated, fail_close, self.clock.clone(), finish_at);
        self.fixtures.0.push(fixture.clone());
        let owner = ReadinessProbeOwner::from_fixture(input, fixture.clone());
        let now = self.now();
        let spec = self.spec(self.campaign.budget.probe_until(now)?)?;
        self.campaign.submit_owner(&self.client, &self.attempt, spec, owner, now)?;
        Ok(fixture)
    }
    pub(super) fn completion(&mut self) -> TestResult<CampaignStep> {
        self.wake.until(&self.client, |batch| {
            let now = Moment::from_nanos(self.clock.load(Ordering::Acquire));
            let matched = self.campaign.attempt.as_ref().is_some_and(|attempt| {
                batch
                    .notifications
                    .iter()
                    .any(|n| n.job_id == attempt.job && n.scope_id == attempt.scope)
            }) || self.campaign.cleanup.as_ref().is_some_and(|job| {
                batch
                    .notifications
                    .iter()
                    .any(|n| n.job_id == job.spec().id && n.scope_id == job.scope_id())
            });
            let step = self.campaign.consume(&batch, now)?;
            Ok(matched.then_some(step))
        })
    }
    pub(super) fn submit_close(&mut self, until: Moment) -> TestResult<JobId> {
        let spec = self.spec(until)?;
        Ok(self.campaign.submit_cleanup(&self.client, &self.cleanup, spec)?)
    }
    pub(super) fn save_batch(&mut self, batch: CompletionBatch) {
        self.saved = Some(batch);
    }
    pub(super) fn wait_terminal_without_consuming(&mut self) -> TestResult<()> {
        let attempt = self.campaign.attempt.as_ref().ok_or("Missing attempt")?;
        let batch = self.wake.until(&self.client, |batch| {
            Ok(batch
                .notifications
                .iter()
                .any(|n| n.job_id == attempt.job && n.scope_id == attempt.scope)
                .then_some(batch))
        })?;
        self.save_batch(batch);
        Ok(())
    }
    pub(super) fn consume_saved(&mut self) -> TestResult<CampaignStep> {
        let batch = self.saved.take().ok_or("Missing actual terminal batch")?;
        Ok(self.campaign.consume(&batch, self.now())?)
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        self.attempt.snapshot().reserved_bytes
    }
    pub(super) fn finish(mut self) -> TestResult<()> {
        for fixture in &self.fixtures.0 {
            fixture.release();
        }
        if self.campaign.attempt.is_some() {
            self.completion()?;
        }
        if self.campaign.cleanup.is_some() {
            self.completion()?;
        }
        self.campaign.cancel(ShutdownBudget::new(self.now())?);
        if !self.campaign.settled() {
            let until = self
                .campaign
                .cleanup_until
                .unwrap_or(self.campaign.shutdown.ok_or("Missing shutdown")?.settle_until);
            self.submit_close(until)?;
            self.completion()?;
        }
        let mut spec = self.spec(Moment::from_nanos(self.now().as_nanos() + 15_000_000_000))?;
        spec.reservation_bytes = FRONTEND_SOURCE_BYTES;
        let mut close = self.source.submit_close(&self.client, &self.cleanup, spec)?;
        let completion = self.wake.until(&self.client, |_| Ok(close.try_take_completion()))?;
        if !matches!(
            completion.outcome,
            sugarsubstitute_contracts::execution::JobOutcome::Success(())
        ) {
            return Err("Source cleanup failed".into());
        }
        drop(completion);
        assert!(self.campaign.settled());
        drop(self.campaign);
        drop(self.source);
        drop(self.lease);
        drop(self.attempt);
        drop(self.cleanup);
        drop(self.client);
        drop(self.executor);
        Ok(())
    }
}
struct FixtureGuard(Vec<Arc<FixtureHttp>>);
impl Drop for FixtureGuard {
    fn drop(&mut self) {
        for fixture in &self.0 {
            fixture.release();
        }
    }
}
