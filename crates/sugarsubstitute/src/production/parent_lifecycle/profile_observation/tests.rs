// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the Parent consumer with actual registry and profile execution jobs.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec,
    Operation, Scope, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::{WindowsProcessRegistry, profiles::PROFILE_OPERATION_BYTES};

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
struct ClockBoundary;
impl Clock for ClockBoundary {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
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
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(in crate::production::parent_lifecycle) struct Fixture {
    owner: Option<ExecutionOwner>,
    pub(in crate::production::parent_lifecycle) client: ExecutionClient,
    pub(in crate::production::parent_lifecycle) scope: Scope,
    wake: Arc<Wake>,
    pub(in crate::production::parent_lifecycle) memory_bytes: u64,
}
impl Fixture {
    pub(in crate::production::parent_lifecycle) fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(ClockBoundary), wake.clone())?;
        let memory_bytes = owner.limits().memory_bytes;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { owner: Some(owner), client, scope, wake, memory_bytes })
    }
    pub(in crate::production::parent_lifecycle) fn terminal(
        &self,
        job: JobId,
    ) -> TestResult<CompletionBatch> {
        loop {
            let prior = *self.wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let batch =
                self.client.drain_completions(NonZeroUsize::new(32).ok_or("Drain bound")?)?;
            if batch.notifications.iter().any(|notice| notice.job_id == job) {
                return Ok(batch);
            }
            let guard = self.wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (_guard, timeout) = self
                .wake
                .changed
                .wait_timeout_while(guard, Duration::from_secs(10), |value| *value == prior)
                .map_err(|_| "Original completion wake poisoned")?;
            if timeout.timed_out() {
                return Err("Original profile job did not notify".into());
            }
        }
    }
    pub(in crate::production::parent_lifecycle) fn finish(&mut self) -> TestResult {
        drop(self.owner.take().ok_or("Original execution owner absent")?);
        Ok(())
    }
    /// Wait for the capacity wake registered by a real rejected admission.
    pub(in crate::production::parent_lifecycle) fn reservation_at_most(
        &self,
        bytes: u64,
    ) -> TestResult {
        let until = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let prior = *self.wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if self.scope.snapshot().reserved_bytes <= bytes {
                return Ok(());
            }
            let remaining = until
                .checked_duration_since(std::time::Instant::now())
                .ok_or("Original capacity wake absent")?;
            let guard = self.wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (_guard, timeout) = self
                .wake
                .changed
                .wait_timeout_while(guard, remaining, |value| *value == prior)
                .map_err(|_| "Original capacity wake poisoned")?;
            if timeout.timed_out() {
                return Err("Original capacity wake absent".into());
            }
        }
    }
}
fn spec(id: u64, bytes: u64) -> TestResult<JobSpec> {
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
fn parent_without_main_proof_drains_original_profile_release() -> TestResult {
    profile_release_after_cleanup_failure(false)
}

#[test]
fn cleanup_admission_failure_retains_original_cause_while_profile_retirement_drains() -> TestResult
{
    profile_release_after_cleanup_failure(true)
}

fn profile_release_after_cleanup_failure(reject_cleanup: bool) -> TestResult {
    let mut fixture = Fixture::new()?;
    let run = super::super::tests::identity().map_err(|cause| cause.to_string())?.run;
    let mut registry_job = WindowsProcessRegistry::submit_new(
        &fixture.client,
        &fixture.scope,
        spec(1, WindowsProcessRegistry::required_reservation_bytes())?,
        run.owner,
    )?;
    fixture.terminal(registry_job.spec().id)?;
    let registry = registry_job.try_take_completion().ok_or("Actual registry result absent")?;
    let JobOutcome::Success(registry_value) = &registry.outcome else {
        return Err("Original registry constructor failed".into());
    };
    let mut profile = WindowsProfileLease::new(
        run,
        std::path::PathBuf::from(r"D:\unopened-parent-release-profile"),
        registry_value,
    )?;
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )?;
    let mut parent = ParentLifecycle::new(
        run,
        sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        sugarsubstitute_contracts::configuration::ProfileId::new("Retire_1".into())?,
        sugarsubstitute_contracts::lifecycle::ConnectionId::new(1)?,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            30_000_000_000,
            1_000_000_000,
        )?,
        &mut Some(crate::production::parent_lifecycle::causes::BackendCauses::new(storage)?),
    )?;
    assert!(parent.profile_pending.is_none());
    let release = spec(2, PROFILE_OPERATION_BYTES)?;
    profile.submit_release(&fixture.client, &fixture.scope, release, registry_value)?;
    let batch = fixture.terminal(release.id)?;
    assert!(profile.snapshot().released && profile.snapshot().pending);
    let mut original = None;
    if reject_cleanup {
        let rejected =
            fixture.client.submit_blocking(&fixture.scope, spec(3, fixture.memory_bytes)?, |_| {
                Ok::<_, std::io::Error>(())
            });
        let cause = match rejected {
            Err(
                cause @ sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated {
                    ..
                },
            ) => cause,
            _ => return Err("Expected actual finite-memory cleanup rejection".into()),
        };
        let cause: super::super::Cause = Box::new(cause);
        original = Some(std::ptr::from_ref(cause.as_ref()).cast::<()>());
        assert!(matches!(
            parent.accept_activity_result(
                Err(cause),
                "frontend_refresh",
                profile.snapshot().pending
            ),
            Some(super::super::ParentActivityFailureOrigin::Coordinator)
        ));
        assert!(parent.closed);
    }
    assert!(parent.profile_completion(&batch, &mut profile)?.is_none());
    assert!(profile.snapshot().retirement_ready(), "Parent failed to consume actual release");
    if let Some(original) = original {
        assert_eq!(
            Some(
                std::ptr::from_ref(parent.failure.as_ref().ok_or("Original cause lost")?.as_ref())
                    .cast::<()>()
            ),
            Some(original)
        );
        assert!(parent.closed, "Profile retirement cannot reopen failed admission");
        assert!(parent.accept_activity_result(Ok(()), "consume_profile", false).is_none());
        assert!(parent.secondary.iter().all(Option::is_none));
    }
    drop(profile);
    drop(registry);
    drop(registry_job);
    fixture.finish()?;
    Ok(())
}
