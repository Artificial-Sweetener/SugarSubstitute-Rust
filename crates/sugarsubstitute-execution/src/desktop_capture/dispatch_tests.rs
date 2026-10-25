// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve actual infrastructure concurrency when native work shares that class.

use super::{control::DesktopCapturePhase, submission, tests, worker::DesktopCaptureActive};
use crate::{
    admission::Core,
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake, spec},
    scope::lock,
    task::{Stop, Work},
};
use std::{io, sync::Arc};
use sugarsubstitute_contracts::execution::*;

/// Select genuine records without spawning a runtime or running finite factories.
#[test]
fn child_native_dispatch_obeys_actual_infrastructure_capacity_and_factory_routing() -> TestResult {
    let now = Moment::from_nanos(0);
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
        crate::runtime::DEFAULT_MEMORY_BYTES,
    );
    let client = crate::ExecutionClient { core: core.clone() };
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut first = client.submit_blocking(
        &scope,
        spec(2, JobClass::InfrastructureLifecycle, 4096)?,
        |_| Ok::<_, io::Error>(()),
    )?;
    let first_running = core
        .take(|class| class == JobClass::InfrastructureLifecycle)
        .ok_or("First actual finite record was not selected")?;
    let mut second = client.submit_blocking(
        &scope,
        spec(3, JobClass::InfrastructureLifecycle, 4096)?,
        |_| Ok::<_, io::Error>(()),
    )?;
    let second_running = core
        .take(|class| class == JobClass::InfrastructureLifecycle)
        .ok_or("Second actual finite record was not selected")?;
    assert_eq!(scope.snapshot().running, 2);
    let mut child_request = tests::request()?;
    child_request.id = JobId::new(4)?;
    let mut child = submission::reserve(
        &core,
        &scope,
        child_request,
        tests::input()?,
        tests::inert_driver(),
        tests::event,
    )?;
    assert_eq!(scope.snapshot().reserved_bytes, 4_194_304 + 8192);
    assert!(core.take_native_at(now).is_none());
    assert!(core.take(|class| class == JobClass::InfrastructureLifecycle).is_none());
    assert_eq!(scope.snapshot().running, 2);

    first_running.job.stop(&first_running.context, Some(now), Stop::Cancelled);
    let first_completion = first.try_take_completion().ok_or("Finite completion missing")?;
    assert!(matches!(first_completion.outcome, JobOutcome::Cancelled));
    drop(first_completion);
    drop(first_running);
    drop(first);
    let (queued, wake) =
        core.take_native_at(now).ok_or("Released capacity did not select child")?;
    core.deliver_capacity_wake(wake);
    assert_eq!(queued.context.spec().class, JobClass::InfrastructureLifecycle);
    assert_eq!(queued.context.job_id(), JobId::new(4)?);
    assert_eq!(scope.snapshot().running, 2);
    let Work::Native(crate::native::NativeWork::DesktopCapture(work)) = queued.work else {
        return Err("Child was routed to a finite factory".into());
    };
    let metadata;
    let mut active = DesktopCaptureActive::new(*work, queued.context, &core, now);
    metadata = active.metadata();
    active.progress(&core, now, true);
    assert!(active.completed());
    let completion = child.job().try_take_completion().ok_or("Child actual terminal missing")?;
    assert!(matches!(completion.outcome, JobOutcome::Failure(_)));
    drop(completion);
    drop(active);
    drop(metadata);
    drop(queued.job);
    drop(child);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    second_running.job.stop(&second_running.context, Some(now), Stop::Cancelled);
    let second_completion =
        second.try_take_completion().ok_or("Second finite completion missing")?;
    assert!(matches!(second_completion.outcome, JobOutcome::Cancelled));
    drop(second_completion);
    drop(second_running);
    drop(second);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Stopped native work must keep its actual class and bypass only dispatch capacity.
#[test]
fn stopped_child_routes_to_native_owner_at_capacity_and_returns_only_unused_loan() -> TestResult {
    let now = Moment::from_nanos(0);
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
        crate::runtime::DEFAULT_MEMORY_BYTES,
    );
    let client = crate::ExecutionClient { core: core.clone() };
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut first = client.submit_blocking(
        &scope,
        spec(2, JobClass::InfrastructureLifecycle, 4096)?,
        |_| Ok::<_, io::Error>(()),
    )?;
    let first_running = core
        .take(|class| class == JobClass::InfrastructureLifecycle)
        .ok_or("First genuine finite record absent")?;
    let mut second = client.submit_blocking(
        &scope,
        spec(3, JobClass::InfrastructureLifecycle, 4096)?,
        |_| Ok::<_, io::Error>(()),
    )?;
    let second_running = core
        .take(|class| class == JobClass::InfrastructureLifecycle)
        .ok_or("Second genuine finite record absent")?;
    let mut request = tests::request()?;
    request.id = JobId::new(4)?;
    let mut child = submission::reserve(
        &core,
        &scope,
        request,
        tests::input()?,
        tests::inert_driver(),
        tests::event,
    )?;
    assert!(core.take_native_at(now).is_none());
    core.cancel_job(request.id);
    core.flush_notifications();
    assert!(core.take(|class| class == JobClass::InfrastructureLifecycle).is_none());
    let (queued, wake) =
        core.take_native_at(now).ok_or("Stopped child was not routed to native owner")?;
    core.deliver_capacity_wake(wake);
    assert_eq!(queued.context.job_id(), request.id);
    assert_eq!(queued.context.spec().class, JobClass::InfrastructureLifecycle);
    let Work::Native(crate::native::NativeWork::DesktopCapture(work)) = queued.work else {
        return Err("Stopped child escaped to generic finite factory".into());
    };
    let retained = work.shared.clone();
    let metadata;
    let mut active = DesktopCaptureActive::new(*work, queued.context, &core, now);
    metadata = active.metadata();
    active.progress(&core, now, true);
    assert!(active.completed());
    assert_eq!(lock(&retained.state).phase, DesktopCapturePhase::UnissuedReturned);
    {
        let cleanup = lock(&retained.cleanup.state);
        assert!(!cleanup.active);
        assert!(!cleanup.native_retired);
        assert!(!cleanup.source_retired);
    }
    {
        let state = lock(&core.state);
        let record =
            state.jobs.get(&request.id).ok_or("Stopped actual record vanished before ACK")?;
        assert!(!record.capture_bundle_issued);
        assert!(!record.capture_event_issued);
        assert!(record.capture_event_identity.is_none());
        assert!(record.native_custody.is_none());
        assert!(record.desktop_capture_custody.is_none());
        assert!(record.native_deadlines.is_none());
    }
    let completion = child.job().try_take_completion().ok_or("Actual stopped terminal missing")?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    drop(active);
    drop(metadata);
    drop(retained);
    drop(queued.job);
    drop(child);
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    first_running.job.stop(&first_running.context, Some(now), Stop::Cancelled);
    drop(first.try_take_completion().ok_or("First finite completion missing")?);
    drop(first_running);
    drop(first);
    second_running.job.stop(&second_running.context, Some(now), Stop::Cancelled);
    drop(second.try_take_completion().ok_or("Second finite completion missing")?);
    drop(second_running);
    drop(second);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
