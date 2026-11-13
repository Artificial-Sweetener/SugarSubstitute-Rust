// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the concurrent first cleanup sample through the actual timer registration.
use super::{custody_tests::SamplingClock, test_support::*, *};
use crate::runtime_test_support::{Gate, ManualClock, TestResult, Wake, spec};
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::*;

/// Release the selected provider gate before the actual execution owner joins.
struct ReleaseSampling(Arc<SamplingClock>);
impl Drop for ReleaseSampling {
    /// Open the external boundary even when an assertion or earlier operation fails.
    fn drop(&mut self) {
        self.0.release.open();
    }
}
/// Observe the original owned job through the actual completion mailbox/wake.
fn completion<T, E>(
    client: &crate::ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> TestResult<JobCompletion<T, E>> {
    loop {
        let previous = wake.count();
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        wake.wait_after(previous)?;
    }
}

#[test]
fn scope_cancel_returns_requested_while_original_sampling_retains_pin_and_deadlines() -> TestResult
{
    let clock = Arc::new(SamplingClock {
        clock: Arc::new(ManualClock::default()),
        target: Mutex::new(None),
        entered: Gate::default(),
        release: Gate::default(),
        fail: false,
    });
    let wake = Arc::new(Wake::default());
    let owner = crate::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let _release_sampling = ReleaseSampling(clock.clone());
    let client = owner.client();
    let ordinary = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let native = Arc::new(client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?);
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    let mut preparation = admission::reserve(
        &client.core,
        &native,
        attach_spec(1)?,
        expected()?,
        boundary.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .map_err(|cause| format!("{cause:?}"))?;
    let completed = completion(&client, &wake, &mut preparation)?;
    let JobOutcome::Success(pin) = completed.outcome else {
        return Err("Actual preparation failed".into());
    };
    drop(completed.memory);
    client.drain_completions(NonZeroUsize::MIN)?;
    clock.clock.advance(3_600_000_000_000);
    let shared = pin.shared.clone();
    struct ClaimCleanup {
        control: control::CaptureCleanupControl,
        clock: Arc<SamplingClock>,
    }
    impl CpuTask<(), std::io::Error> for ClaimCleanup {
        fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, std::io::Error> {
            *lock(&self.clock.target) = Some(std::thread::current().id());
            self.control.request_cleanup();
            Ok(CpuStep::Complete(()))
        }
    }
    let control = control::CaptureCleanupControl { custody: shared.custody.clone() };
    let sampler = clock.clone();
    let mut cancellation =
        client.submit_cpu(&ordinary, spec(2, JobClass::CpuDecode, 4096)?, move |_| {
            ClaimCleanup { control, clock: sampler }
        })?;
    clock.entered.wait()?;
    assert!(shared.custody.stopped());
    assert!(matches!(shared.custody.snapshot(), CaptureCleanupSnapshot::Requested));
    native.cancel();
    assert!(native.snapshot().cancellation_requested);
    assert!(matches!(shared.custody.snapshot(), CaptureCleanupSnapshot::Requested));
    assert!(shared.custody.held());
    assert_eq!(native.snapshot().reserved_bytes, 16_384);
    assert!(!boundary.physical_closed.load(Ordering::Acquire));
    clock.release.open();
    clock.clock.wait_until_registered(Moment::from_nanos(3_610_000_000_000))?;
    let CaptureCleanupSnapshot::Latched { budget: original, .. } = shared.custody.snapshot() else {
        return Err("Actual timer registration did not retain original sampled budget".into());
    };
    assert_eq!(original.graceful_until, Moment::from_nanos(3_610_000_000_000));
    assert_eq!(original.settle_until, Moment::from_nanos(3_615_000_000_000));
    assert!(matches!(
        completion(&client, &wake, &mut cancellation)?.outcome,
        JobOutcome::Success(())
    ));
    clock.clock.advance(1_000_000_000);
    let _retirement = pin.release();
    let CaptureCleanupSnapshot::Latched { budget, .. } = shared.custody.snapshot() else {
        return Err("Release replaced original cleanup".into());
    };
    assert_eq!(budget, original);
    boundary.settle()?;
    boundary.source_retired.wait()?;
    drop(preparation);
    drop(cancellation);
    drop(owner);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 1);
    Ok(())
}
