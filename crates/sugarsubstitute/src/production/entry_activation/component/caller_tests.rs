// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise final callers while the real release job remains unconsumed.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::{WindowsProcessRegistry, profiles::PROFILE_OPERATION_BYTES};

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut count = self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *count += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    /// Drain the genuine matching terminal notification without taking its outcome.
    fn terminal(&self, client: &ExecutionClient, id: JobId) -> Result<CompletionBatch> {
        loop {
            let prior = *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let batch = client.drain_completions(NonZeroUsize::new(32).ok_or("Drain bound")?)?;
            if batch.notifications.iter().any(|notice| notice.job_id == id) {
                return Ok(batch);
            }
            let count = self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (count, timeout) = self
                .changed
                .wait_timeout_while(count, Duration::from_secs(10), |count| *count == prior)
                .map_err(|_| "Actual completion wake poisoned")?;
            drop(count);
            if timeout.timed_out() {
                return Err("Actual terminal notification absent".into());
            }
        }
    }
}
fn specification(id: u64, bytes: u64) -> Result<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    })
}

#[test]
fn component_final_callers_retain_actual_released_pending_operation_and_first_failure() -> Result<()>
{
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let mut registry_job = WindowsProcessRegistry::submit_new(
        &client,
        &scope,
        specification(1, WindowsProcessRegistry::required_reservation_bytes())?,
        run.owner,
    )?;
    let registry_batch = wake.terminal(&client, registry_job.spec().id)?;
    let registry = registry_job.try_take_completion().ok_or("Actual registry result absent")?;
    let JobOutcome::Success(registry_value) = &registry.outcome else {
        return Err("Actual registry construction failed".into());
    };
    let mut profile = WindowsProfileLease::new(
        run,
        PathBuf::from(r"D:\unopened-component-profile"),
        registry_value,
    )?;
    // The production never-issued release owns no filesystem or predecessor authority.
    let id = profile.submit_release(
        &client,
        &scope,
        specification(2, PROFILE_OPERATION_BYTES)?,
        registry_value,
    )?;
    let release_batch = wake.terminal(&client, id)?;
    assert!(profile.snapshot().released && profile.snapshot().pending);
    assert!(!profile.snapshot().retirement_ready());

    let mut input = Some(projection::ProjectionInput::without_request());
    let mut projection_job = client.submit_blocking_owned_box(
        &scope,
        specification(3, projection::PROJECTION_BYTES)?,
        &mut input,
        projection::project,
    )?;
    let projection_batch = wake.terminal(&client, projection_job.spec().id)?;
    let projected =
        projection_job.try_take_completion().ok_or("Actual projection result absent")?;
    assert!(matches!(&projected.outcome, JobOutcome::Failure(_)));
    let storage = projected.memory.claim((size_of::<Campaign>() + 1_048_576) as u64)?;
    let mut component = ProfileComponent::new(
        PathBuf::from(r"D:\unopened-component.exe"),
        PathBuf::from(r"D:\unopened-installation"),
    )?;
    component.campaign = Some(Box::new(Campaign {
        projection: projected,
        custody: None,
        profile: Some(profile),
        watch: None,
        ticket: None,
        publication_issued: false,
        reply_until: None,
        replied: false,
        started: false,
        exited: false,
        registry_retired: false,
        released: false,
        _storage: storage,
    }));
    let budget = ShutdownBudget {
        started: Moment::from_nanos(1),
        graceful_until: Moment::from_nanos(10_000_000_000),
        settle_until: Moment::from_nanos(15_000_000_000),
    };
    component.request_close(Some(budget))?;
    // Reproduce the prior failure-close completion latch while the original job is retained.
    component.complete = true;
    component.preserve("original component failure".into());
    assert!(!component.profile_retirement_ready());
    assert!(component.finish().is_err());
    assert!(!component.poll_close(&projection_batch, None, budget)?);
    assert_eq!(component.shutdown_budget(), Some(budget));
    assert_eq!(
        component.failure.as_ref().ok_or("First cause absent")?.to_string(),
        "original component failure"
    );
    let first = component.failure.take().ok_or("First cause absent")?;
    assert!(!component.normal_close_requested());
    component.failure = Some(first);
    let campaign = component.campaign.as_mut().ok_or("Original campaign absent")?;
    let actual = campaign
        .profile
        .as_mut()
        .ok_or("Original profile absent")?
        .consume(&release_batch)?
        .ok_or("Original release result absent")?;
    assert_eq!(actual.job, id);
    assert_eq!(actual.kind, TerminalKind::Success);
    assert!(actual.failure.is_none());
    assert!(component.profile_retirement_ready());
    assert!(component.poll_close(&registry_batch, None, budget)?);
    component.finish()?;
    assert_eq!(
        component.failure.as_ref().ok_or("First cause absent")?.to_string(),
        "original component failure"
    );
    drop(component);
    drop(registry);
    drop(projection_job);
    drop(registry_job);
    drop(executor);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
