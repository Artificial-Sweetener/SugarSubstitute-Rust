// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise transaction admission, actual completion and retained claims through a fake native boundary.
use super::*;
use crate::installed_frontend::state::{Inspection, SourcePort};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobContext, JobId, JobPhase,
    JobSpec, Moment, Operation, Scope, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
#[derive(Default)]
struct TestClock(AtomicU64);
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
    sequence: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *crate::registry::lock(&self.sequence) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn until(&self, client: &ExecutionClient, mut ready: impl FnMut() -> bool) -> TestResult {
        loop {
            let revision = *crate::registry::lock(&self.sequence);
            client
                .drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid drain bound")?)?;
            if ready() {
                return Ok(());
            }
            let guard = crate::registry::lock(&self.sequence);
            drop(
                self.changed
                    .wait_while(guard, |sequence| *sequence == revision)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }
}
struct Boundary {
    clock: Arc<TestClock>,
    late: bool,
    fail_close: bool,
    releases: AtomicU64,
    lease: Mutex<Option<InstalledFrontendLease>>,
}
impl SourcePort for Boundary {
    fn inspect(
        &self,
        _: &FrontendInstallationInput,
        state: &mut Inspection,
        _: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        let mut manifest = inventory();
        manifest::enroll(&mut manifest, asset("/index.html", 1, 2), false)?;
        manifest::enroll(&mut manifest, asset("/extensions/SugarCubes/a.js", 2, 2), true)?;
        state.manifest = Some(Arc::new(manifest));
        if self.late {
            self.clock.0.store(1000, Ordering::Release);
        }
        Ok(())
    }
    fn close(&self, _state: &mut Inspection, _: &JobContext) -> Result<(), FrontendSourceError> {
        if crate::registry::lock(&self.lease)
            .as_ref()
            .is_some_and(|lease| lease.validate_live().is_ok())
        {
            return Err(FrontendSourceError::invalid("fake_close_entered_before_retirement"));
        }
        if self.releases.fetch_add(1, Ordering::AcqRel) != 0 && self.fail_close {
            return Err(FrontendSourceError::new(
                "fake_close",
                std::io::Error::other("original-close-cause"),
            ));
        }
        Ok(())
    }
}
struct Harness {
    owner: InstalledFrontendOwner,
    executor: ExecutionOwner,
    client: ExecutionClient,
    scope: Arc<Scope>,
    wake: Arc<Wake>,
    clock: Arc<TestClock>,
    boundary: Arc<Boundary>,
}
impl Harness {
    fn new(late: bool, fail_close: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let clock = Arc::new(TestClock::default());
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = executor.client();
        let scope = Arc::new(
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        let input = FrontendInstallationInput::new(
            "E:\\fixture\\package".into(),
            "E:\\fixture\\METADATA".into(),
            "E:\\fixture\\cube".into(),
            "E:\\fixture\\main.py".into(),
        )?;
        let mut owner = InstalledFrontendOwner::new(input);
        let boundary = Arc::new(Boundary {
            clock: clock.clone(),
            late,
            fail_close,
            releases: AtomicU64::new(0),
            lease: Mutex::new(None),
        });
        owner.port = boundary.clone();
        Ok(Self { owner, executor, client, scope, wake, clock, boundary })
    }
    fn spec(id: u64, deadline: u64) -> Result<JobSpec, Box<dyn std::error::Error>> {
        Ok(JobSpec {
            id: JobId::new(id)?,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(deadline)),
            reservation_bytes: FRONTEND_SOURCE_BYTES,
            replacement: None,
        })
    }
    fn inspect(&mut self) -> TestResult {
        self.owner.submit_inspection(&self.client, &self.scope, Self::spec(1, 1000)?)?;
        self.wake.until(&self.client, || {
            self.owner
                .pending
                .as_ref()
                .is_some_and(|job| matches!(job.status().phase, JobPhase::Terminal(_)))
        })
    }
    /// Admit closure while the original inspection continues to fund retained native custody.
    fn close_spec(id: u64, deadline: u64) -> Result<JobSpec, Box<dyn std::error::Error>> {
        let mut spec = Self::spec(id, deadline)?;
        spec.reservation_bytes = FRONTEND_SOURCE_CLOSE_BYTES;
        Ok(spec)
    }
    fn close(
        &self,
    ) -> Result<
        sugarsubstitute_contracts::execution::JobCompletion<(), FrontendSourceError>,
        Box<dyn std::error::Error>,
    > {
        let mut job =
            self.owner.submit_close(&self.client, &self.scope, Self::close_spec(2, 2000)?)?;
        self.wake.until(&self.client, || matches!(job.status().phase, JobPhase::Terminal(_)))?;
        job.try_take_completion().ok_or_else(|| "Missing typed cleanup".into())
    }
}
#[test]
fn actual_success_lease_keeps_original_storage_claim_after_explicit_close() -> TestResult {
    let mut harness = Harness::new(false, false)?;
    harness.inspect()?;
    let lease = harness.owner.take_inspection()?.ok_or("Missing lease")?;
    assert_eq!(lease.manifest().assets().len(), 2);
    lease.validate_live()?;
    *crate::registry::lock(&harness.boundary.lease) = Some(lease.clone());
    let completion = harness.close()?;
    assert!(matches!(
        completion.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    assert!(lease.validate_live().is_err());
    assert_eq!(lease._claim.bytes(), FRONTEND_SOURCE_BYTES);
    assert!(harness.scope.snapshot().reserved_bytes >= FRONTEND_SOURCE_BYTES);
    drop(completion);
    drop(lease);
    drop(harness.owner);
    drop(harness.scope);
    drop(harness.executor);
    Ok(())
}
#[test]
fn rejected_valid_close_request_permanently_retires_without_releasing_custody() -> TestResult {
    let mut harness = Harness::new(false, false)?;
    harness.inspect()?;
    let lease = harness.owner.take_inspection()?.ok_or("Missing lease")?;
    lease.validate_live()?;
    harness.scope.cancel();
    assert!(
        harness
            .owner
            .submit_close(&harness.client, &harness.scope, Harness::close_spec(2, 2000)?)
            .is_err()
    );
    assert!(lease.validate_live().is_err());
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_some());
    assert!(harness.scope.snapshot().reserved_bytes >= FRONTEND_SOURCE_BYTES);
    let cleanup =
        harness.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut job =
        harness.owner.submit_close(&harness.client, &cleanup, Harness::close_spec(3, 2000)?)?;
    harness.wake.until(&harness.client, || matches!(job.status().phase, JobPhase::Terminal(_)))?;
    assert!(matches!(
        job.try_take_completion().ok_or("Missing cleanup")?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    assert!(lease.validate_live().is_err());
    Ok(())
}
#[test]
fn late_native_manifest_is_not_ready_after_discarded_timed_out_delivery() -> TestResult {
    let mut harness = Harness::new(true, false)?;
    harness.inspect()?;
    assert!(harness.owner.take_inspection().is_err());
    let retained = crate::registry::lock(&harness.owner.shared.retained);
    assert!(retained.as_ref().is_some_and(|state| state.manifest.is_some()));
    drop(retained);
    assert!(harness.scope.snapshot().reserved_bytes >= FRONTEND_SOURCE_BYTES);
    assert!(matches!(
        harness.close()?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    Ok(())
}
#[test]
fn late_application_delivery_rejects_original_success_without_resetting_deadline() -> TestResult {
    let mut harness = Harness::new(false, false)?;
    harness.inspect()?;
    harness.clock.0.store(1000, Ordering::Release);
    assert!(harness.owner.take_inspection().is_err());
    assert!(matches!(
        harness.close()?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    Ok(())
}
#[test]
fn cleanup_failure_preserves_partial_state_charge_and_original_cause() -> TestResult {
    use std::error::Error;
    let mut harness = Harness::new(false, true)?;
    harness.inspect()?;
    let lease = harness.owner.take_inspection()?.ok_or("Missing lease")?;
    let completion = harness.close()?;
    let sugarsubstitute_contracts::execution::JobOutcome::Failure(cause) = completion.outcome
    else {
        return Err("Expected closure failure".into());
    };
    assert_eq!(cause.stage(), "fake_close");
    assert_eq!(cause.source().ok_or("Missing cause")?.to_string(), "original-close-cause");
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_some());
    assert!(lease.validate_live().is_err());
    Ok(())
}

mod stopped;

mod progress;

#[test]
fn completed_source_lease_survives_elapsed_preparation_until_actual_explicit_close() -> TestResult {
    let mut harness = Harness::new(false, false)?;
    harness.inspect()?;
    let lease = harness.owner.take_inspection()?.ok_or("Missing completed source lease")?;
    let original_assets = lease.manifest().assets().len();
    harness.clock.0.store(600_000_000_000, Ordering::Release);
    lease.validate_live()?;
    assert_eq!(lease.manifest().assets().len(), original_assets);
    assert_eq!(lease._claim.bytes(), FRONTEND_SOURCE_BYTES);
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_some());
    *crate::registry::lock(&harness.boundary.lease) = Some(lease.clone());
    let mut job = harness.owner.submit_close(
        &harness.client,
        &harness.scope,
        Harness::close_spec(2, 615_000_000_000)?,
    )?;
    assert!(lease.validate_live().is_err());
    harness.wake.until(&harness.client, || matches!(job.status().phase, JobPhase::Terminal(_)))?;
    let completion = job.try_take_completion().ok_or("Missing actual source cleanup")?;
    assert!(matches!(
        completion.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_none());
    assert_eq!(lease._claim.bytes(), FRONTEND_SOURCE_BYTES);
    Ok(())
}

#[test]
fn completed_snapshot_releases_inspection_before_delivery_and_final_close_is_idempotent()
-> TestResult {
    let mut harness = Harness::new(false, false)?;
    harness.inspect()?;
    assert_eq!(harness.boundary.releases.load(Ordering::Acquire), 1);
    let lease = harness.owner.take_inspection()?.ok_or("Missing completed snapshot")?;
    lease.validate_live()?;
    {
        let retained = crate::registry::lock(&harness.owner.shared.retained);
        let retained = retained.as_ref().ok_or("Snapshot owner missing")?;
        assert!(retained.handles.is_empty());
        assert_eq!(retained.search, 0);
        assert!(retained.manifest.is_some());
    }
    assert!(matches!(
        harness.close()?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    assert!(lease.validate_live().is_err());
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_none());
    assert_eq!(harness.boundary.releases.load(Ordering::Acquire), 2);
    let mut again = harness.owner.submit_close(
        &harness.client,
        &harness.scope,
        Harness::close_spec(3, 2000)?,
    )?;
    harness
        .wake
        .until(&harness.client, || matches!(again.status().phase, JobPhase::Terminal(_)))?;
    assert!(matches!(
        again.try_take_completion().ok_or("Missing repeated close")?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(())
    ));
    assert_eq!(harness.boundary.releases.load(Ordering::Acquire), 2);
    Ok(())
}

mod native_snapshot;

mod cleanup_memory;

#[path = "ownership/reload.rs"]
mod reload;
