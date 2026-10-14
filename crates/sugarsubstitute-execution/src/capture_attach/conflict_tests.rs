// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove full-run conflicts follow actual custody rather than result or scope identity.
use super::{test_support::*, *};
use crate::runtime_test_support::{Rig, TestResult};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::{execution::*, incident::BuildIdentity, lifecycle::*};
use types::*;

pub(super) fn expectation(run: RunKey) -> TestResult<CaptureAttachExpectation> {
    let original = expected()?;
    Ok(CaptureAttachExpectation::new(
        run.owner,
        sugarsubstitute_contracts::crash::CaptureTarget::Desktop(run),
        &BuildIdentity::new("test-build")?,
        original.creator(),
        original.handler(),
        original.attach_budget(),
    )?)
}
pub(super) fn another_scope(rig: &Rig, id: u64) -> TestResult<Scope> {
    Ok(rig.client.create_native_scope(
        ScopeSpec { id: ScopeId::new(id)?, deadline: None },
        Some(&rig.scope),
    )?)
}
pub(super) fn rejected(
    rig: &Rig,
    scope: &Scope,
    request: JobSpec,
    original: CaptureAttachExpectation,
) -> TestResult {
    let boundary = Arc::new(Boundary::default());
    let source: Arc<dyn CaptureEndpointSource> = boundary.clone();
    let before = scope.snapshot();
    let count = lock(&rig.client.core.state).jobs.len();
    let rejection = admission::reserve(
        &rig.client.core,
        scope,
        request,
        original,
        source.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .err()
    .ok_or("Duplicate run was admitted")?;
    assert!(
        matches!(rejection.error, admission::CaptureAttachAdmissionError::TargetAlreadyReserved(run) if run == original.target())
    );
    assert_eq!(rejection.expected, original);
    assert!(Arc::ptr_eq(&rejection.source, &source));
    assert_eq!(scope.snapshot(), before);
    assert_eq!(lock(&rig.client.core.state).jobs.len(), count);
    assert_eq!(boundary.observations.load(Ordering::Acquire), 0);
    assert_eq!(boundary.starts.load(Ordering::Acquire), 0);
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 0);
    assert_eq!(format!("{:?}", rejection.error), "TargetAlreadyReserved { redacted }");
    assert!(std::error::Error::source(&rejection.error).is_none());
    Ok(())
}
#[test]
fn running_terminal_consumption_ack_and_uncertain_cleanup_keep_same_run_conflict() -> TestResult {
    let rig = Rig::new()?;
    let first = scope(&rig)?;
    let second = another_scope(&rig, 3)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    let mut job = submit(&rig, &first, &boundary)?;
    boundary.started.wait()?;
    // Conflict precedes even a duplicate JobId and a capacity-failing charge.
    let mut request = attach_spec(1)?;
    request.reservation_bytes = u64::MAX;
    rejected(&rig, &second, request, expected()?)?;
    boundary.ready.store(true, Ordering::Release);
    boundary.signal()?;
    let pin = take_pin(&rig, &mut job)?;
    assert_eq!(first.snapshot().terminal_unobserved, 0);
    rejected(&rig, &second, attach_spec(999)?, expected()?)?;
    let mut retirement = pin.release();
    boundary.retiring.wait()?;
    let CaptureCleanupSnapshot::Latched { budget, .. } = retirement.shared.custody.snapshot()
    else {
        return Err("Cleanup was not latched".into());
    };
    rig.clock.wait_until_registered(budget.graceful_until)?;
    rig.clock.advance(10_000_000_000);
    rig.clock.wait_until_registered(budget.settle_until)?;
    rig.clock.advance(5_000_000_000);
    assert!(matches!(
        take_retirement_report(&rig, &mut retirement)?,
        Err(CaptureAttachFailure::Uncertain)
    ));
    rejected(&rig, &second, attach_spec(999)?, expected()?)?;
    assert_eq!(first.snapshot().reserved_bytes, 16_384);
    boundary.settle()?;
    boundary.source_retired.wait()?;
    loop {
        let previous = rig.wake.count();
        if !retirement.shared.custody.held() {
            break;
        }
        rig.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if !retirement.shared.custody.held() {
            break;
        }
        rig.wake.wait_after(previous)?;
    }
    // Failed 999 admissions advanced no identity high-water. Actual settled
    // custody permits id2; historical replay belongs to the real source ledger.
    let next = Arc::new(Boundary::default());
    let _release_next = ReleaseBoundary::new(&rig.client.core, &next);
    next.ready.store(true, Ordering::Release);
    let mut second_job = admission::reserve(
        &rig.client.core,
        &second,
        attach_spec(2)?,
        expected()?,
        next.clone(),
        Box::new(Lease(next.clone())),
    )
    .map_err(|cause| format!("{cause:?}"))?;
    let next_pin = take_pin(&rig, &mut second_job)?;
    let _next_retirement = next_pin.release();
    next.settle()?;
    next.source_retired.wait()?;
    assert!(retirement.try_take_report().is_none());
    assert!(job.try_take_completion().is_none());
    drop(second_job);
    drop(job);
    drop(rig.owner);
    assert_eq!(first.snapshot().reserved_bytes, 0);
    assert_eq!(second.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn conflict_matches_complete_namespace_epoch_instance_and_generation() -> TestResult {
    let rig = Rig::new()?;
    let first = scope(&rig)?;
    let second = another_scope(&rig, 3)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    let mut job = submit(&rig, &first, &boundary)?;
    let pin = take_pin(&rig, &mut job)?;
    let original = expected()?.target().desktop().ok_or("Desktop fixture target absent")?;
    let mut namespace = original;
    namespace.owner.namespace = NamespaceId::new([99; 32]);
    let mut epoch = original;
    epoch.owner.epoch = CoordinatorEpoch::new([99; 16])?;
    let mut instance = original;
    instance.instance = InstanceId::new(2)?;
    let mut generation = original;
    generation.generation = RunGeneration::new(2)?;
    for (offset, run) in [namespace, epoch, instance, generation].into_iter().enumerate() {
        let next = Arc::new(Boundary::default());
        let _release_next = ReleaseBoundary::new(&rig.client.core, &next);
        next.ready.store(true, Ordering::Release);
        let mut other = admission::reserve(
            &rig.client.core,
            &second,
            attach_spec(offset as u64 + 2)?,
            expectation(run)?,
            next.clone(),
            Box::new(Lease(next.clone())),
        )
        .map_err(|cause| format!("{cause:?}"))?;
        let next_pin = take_pin(&rig, &mut other)?;
        let _retirement = next_pin.release();
        next.settle()?;
        next.source_retired.wait()?;
        drop(other);
    }
    let _retirement = pin.release();
    boundary.settle()?;
    boundary.source_retired.wait()?;
    drop(job);
    drop(rig.owner);
    assert_eq!(first.snapshot().reserved_bytes, 0);
    assert_eq!(second.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn physical_close_does_not_release_conflict_before_actual_source_retirement() -> TestResult {
    let rig = Rig::new()?;
    let first = scope(&rig)?;
    let second = another_scope(&rig, 3)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    boundary.source_blocked.store(true, Ordering::Release);
    let mut job = submit(&rig, &first, &boundary)?;
    let pin = take_pin(&rig, &mut job)?;
    let shared = pin.shared.clone();
    let mut retirement = pin.release();
    boundary.settle()?;
    boundary.source_attempted.wait()?;
    assert!(boundary.physical_closed.load(Ordering::Acquire));
    assert!(!lock(&shared.custody.state).source_retired);
    assert!(shared.custody.held());
    rejected(&rig, &second, attach_spec(2)?, expected()?)?;
    assert_eq!(first.snapshot().reserved_bytes, 16_384);
    let cause = rig
        .client
        .drain_completions(std::num::NonZeroUsize::MIN)
        .err()
        .ok_or("Actual source retirement failure was not reported")?;
    assert!(std::error::Error::source(&cause).is_some());
    boundary.source_blocked.store(false, Ordering::Release);
    boundary.signal()?;
    boundary.source_retired.wait()?;
    assert_eq!(take_retirement_report(&rig, &mut retirement)??.target(), expected()?.target());
    assert!(!shared.custody.held());
    drop(job);
    drop(rig.owner);
    assert_eq!(first.snapshot().reserved_bytes, 0);
    Ok(())
}
