// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real terminal acknowledgment, late cleanup and source retirement association.
use super::{test_support::*, *};
use crate::runtime_test_support::{Rig, TestResult};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::execution::*;

#[test]
fn twenty_second_preparation_retains_acknowledged_native_capacity_until_checked_close() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    let mut job = submit(&rig, &scope, &boundary)?;
    boundary.started.wait()?;
    rig.clock.advance(20_000_000_000);
    boundary.ready.store(true, Ordering::Release);
    boundary.signal()?;
    let pin = take_pin(&rig, &mut job)?;
    assert!(matches!(pin.shared.custody.snapshot(), CaptureCleanupSnapshot::Dormant));
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(scope.snapshot().reserved_bytes, 16_384);
    {
        let state = lock(&rig.client.core.state);
        let record =
            state.jobs.get(&JobId::new(1)?).ok_or("Acknowledgment removed held custody")?;
        assert!(record.acknowledged && record.held_native());
        assert!(record.native_custody.as_ref().is_some_and(|custody| custody.active()));
    }
    let shared = pin.shared.clone();
    let mut retirement = pin.release();
    let CaptureCleanupSnapshot::Latched { budget, .. } = shared.custody.snapshot() else {
        return Err("First actual release did not latch cleanup".into());
    };
    assert_eq!(budget.graceful_until, Moment::from_nanos(30_000_000_000));
    assert_eq!(budget.settle_until, Moment::from_nanos(35_000_000_000));
    boundary.retiring.wait()?;
    assert!(retirement.try_take_report().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 16_384);
    boundary.settle()?;
    boundary.source_retired.wait()?;
    let report = take_retirement_report(&rig, &mut retirement)??;
    assert_eq!(report.run(), expected()?.run());
    assert_eq!(report.job_id(), JobId::new(1)?);
    assert!(retirement.try_take_report().is_none());
    drop(job);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn long_lived_prepared_pin_latches_first_scope_shutdown_without_launch_time_cleanup() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    let mut job = submit(&rig, &scope, &boundary)?;
    let pin = take_pin(&rig, &mut job)?;
    rig.clock.advance(3_600_000_000_000);
    assert!(matches!(pin.shared.custody.snapshot(), CaptureCleanupSnapshot::Dormant));
    scope.cancel();
    let CaptureCleanupSnapshot::Latched { budget: original, .. } = pin.shared.custody.snapshot()
    else {
        return Err("Scope shutdown failed to stop terminal preparation custody".into());
    };
    assert_eq!(original.graceful_until, Moment::from_nanos(3_610_000_000_000));
    assert_eq!(original.settle_until, Moment::from_nanos(3_615_000_000_000));
    rig.clock.advance(1_000_000_000);
    let shared = pin.shared.clone();
    let _retirement = pin.release();
    let CaptureCleanupSnapshot::Latched { budget, .. } = shared.custody.snapshot() else {
        return Err("Release replaced original cleanup state".into());
    };
    assert_eq!(budget, original);
    boundary.settle()?;
    boundary.source_retired.wait()?;
    drop(job);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn cleanup_total_publishes_uncertain_once_and_late_close_preserves_original_terminal() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    let mut job = submit(&rig, &scope, &boundary)?;
    let pin = take_pin(&rig, &mut job)?;
    let shared = pin.shared.clone();
    let mut retirement = pin.release();
    let CaptureCleanupSnapshot::Latched { budget, .. } = shared.custody.snapshot() else {
        return Err("Cleanup was not latched".into());
    };
    rig.clock.wait_until_registered(budget.graceful_until)?;
    rig.clock.advance(10_000_000_000);
    rig.clock.wait_until_registered(budget.settle_until)?;
    rig.clock.advance(5_000_000_000);
    let failure = take_retirement_report(&rig, &mut retirement)?;
    assert!(matches!(failure, Err(types::CaptureAttachFailure::Uncertain)));
    assert_eq!(scope.snapshot().reserved_bytes, 16_384);
    assert!(!boundary.physical_closed.load(Ordering::Acquire));
    assert!(job.try_take_completion().is_none());
    boundary.settle()?;
    boundary.source_retired.wait()?;
    drop(job);
    drop(rig.owner);
    assert!(retirement.try_take_report().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert!(lock(&shared.custody.state).native_retired);
    assert!(lock(&shared.custody.state).source_retired);
    Ok(())
}

struct SamplingClock {
    clock: Arc<crate::runtime_test_support::ManualClock>,
    target: Mutex<Option<std::thread::ThreadId>>,
    entered: crate::runtime_test_support::Gate,
    release: crate::runtime_test_support::Gate,
}
impl Clock for SamplingClock {
    fn now(&self) -> Moment {
        let block = {
            let mut target = lock(&self.target);
            if *target == Some(std::thread::current().id()) {
                target.take().is_some()
            } else {
                false
            }
        };
        if block {
            self.entered.open();
            if let Err(cause) = self.release.wait() {
                std::panic::resume_unwind(Box::new(cause.to_string()));
            }
            std::panic::resume_unwind(Box::new(TimeError::Regression));
        }
        crate::runtime_test_support::ClockPort(self.clock.clone()).now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        crate::runtime_test_support::ClockPort(self.clock.clone()).sleep_until(deadline)
    }
}
#[test]
fn first_cleanup_claim_retains_charge_and_cause_when_native_retirement_wins_sampling() -> TestResult
{
    use crate::runtime_test_support::{Gate, ManualClock, Wake, spec};
    use std::num::NonZeroUsize;
    let clock = Arc::new(SamplingClock {
        clock: Arc::new(ManualClock::default()),
        target: Mutex::new(None),
        entered: Gate::default(),
        release: Gate::default(),
    });
    let wake = Arc::new(Wake::default());
    let owner = crate::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let ordinary = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let native =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    boundary.close_allowed.store(true, Ordering::Release);
    let mut preparation = admission::reserve(
        &client.core,
        &native,
        attach_spec(1)?,
        expected()?,
        boundary.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .map_err(|cause| format!("{cause:?}"))?;
    let completion = loop {
        let previous = wake.count();
        if let Some(completion) = preparation.try_take_completion() {
            break completion;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = preparation.try_take_completion() {
            break completion;
        }
        wake.wait_after(previous)?;
    };
    let JobOutcome::Success(pin) = completion.outcome else {
        return Err("Preparation failed".into());
    };
    drop(completion.memory);
    client.drain_completions(NonZeroUsize::MIN)?;
    let shared = pin.shared.clone();
    let worker_clock = clock.clone();
    // CPU job ownership supplies the overlapping provider call; no test thread.
    struct ReleasePin {
        pin: Option<CaptureAttachLaunchPin>,
        clock: Arc<SamplingClock>,
    }
    impl CpuTask<CaptureAttachRetirement, std::io::Error> for ReleasePin {
        fn step(
            &mut self,
            _: &JobContext,
        ) -> Result<CpuStep<CaptureAttachRetirement>, std::io::Error> {
            *lock(&self.clock.target) = Some(std::thread::current().id());
            let pin = self
                .pin
                .take()
                .ok_or_else(|| std::io::Error::other("Capture pin was already released"))?;
            Ok(CpuStep::Complete(pin.release()))
        }
    }
    let mut release_job =
        client.submit_cpu(&ordinary, spec(2, JobClass::CpuDecode, 4096)?, move |_| ReleasePin {
            pin: Some(pin),
            clock: worker_clock,
        })?;
    struct ReleaseSampling(Arc<SamplingClock>);
    impl Drop for ReleaseSampling {
        fn drop(&mut self) {
            self.0.release.open();
        }
    }
    let release_sampling = ReleaseSampling(clock.clone());
    clock.entered.wait()?;
    boundary.source_retired.wait()?;
    loop {
        let previous = wake.count();
        if lock(&shared.custody.state).source_retired {
            break;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if lock(&shared.custody.state).source_retired {
            break;
        }
        wake.wait_after(previous)?;
    }
    assert!(lock(&shared.custody.state).native_retired);
    assert!(lock(&shared.custody.state).source_retired);
    assert!(matches!(shared.custody.snapshot(), CaptureCleanupSnapshot::Requested));
    assert!(shared.custody.held());
    assert_eq!(native.snapshot().reserved_bytes, 16_384);
    assert!(lock(&client.core.state).jobs.contains_key(&JobId::new(1)?));
    let duplicate_source: Arc<dyn types::CaptureEndpointSource> = boundary.clone();
    let rejection = admission::reserve(
        &client.core,
        &native,
        attach_spec(3)?,
        expected()?,
        duplicate_source.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .err()
    .ok_or("Sampling custody admitted the same run")?;
    assert!(
        matches!(rejection.error, admission::CaptureAttachAdmissionError::RunAlreadyReserved(run)
        if run == expected()?.run())
    );
    assert!(Arc::ptr_eq(&rejection.source, &duplicate_source));
    assert_eq!(rejection.expected, expected()?);
    assert_eq!(boundary.starts.load(Ordering::Acquire), 1);
    clock.release.open();
    let mut observed_failure = None;
    let result = loop {
        let previous = wake.count();
        if let Some(completion) = release_job.try_take_completion() {
            break completion;
        }
        if let Err(cause) = client.drain_completions(NonZeroUsize::MIN) {
            observed_failure.get_or_insert(cause);
        }
        if let Some(completion) = release_job.try_take_completion() {
            break completion;
        }
        wake.wait_after(previous)?;
    };
    drop(result);
    let CaptureCleanupSnapshot::ClockUnavailable(cause) = shared.custody.snapshot() else {
        return Err("Original provider failure was lost after physical retirement".into());
    };
    assert_eq!(cause.stage(), "capture_cleanup_clock");
    assert!(!shared.custody.held());
    let original = observed_failure
        .or_else(|| client.drain_completions(NonZeroUsize::MIN).err())
        .ok_or("Original clock failure did not reach bounded owner channel")?;
    assert!(std::error::Error::source(&original).is_some());
    drop(release_sampling);
    drop(preparation);
    drop(release_job);
    drop(owner);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 1);
    Ok(())
}
