// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise preparation admission and hidden-state behavior through the real execution port.

use super::*;
use std::{convert::Infallible, time::Duration};
use sugarsubstitute_contracts::execution::{JobId, ScopeId, ScopeSpec};
use sugarsubstitute_splash::PaperFlip;

#[path = "preparation_test_support.rs"]
mod support;

/// Native input geometry must describe accepted pixels even when a later sample is queued.
#[test]
fn published_appearance_belongs_to_completed_frame_not_latest_request() -> Result<(), Box<dyn Error>>
{
    let (mut driver, wake, mut owner) = support::started()?;
    let initial = support::prepare(&mut driver, &wake, 0.0)?;
    let appearance = Appearance { reveal: 0.25, ..Appearance::demo() };
    driver.request(
        initial.shape(),
        appearance,
        PaperFlip::new(1).frame(Duration::ZERO),
        ProgressFrame { completed: 0.4, activity: None },
        None,
    )?;
    support::terminal(&driver, &wake)?;
    driver.request(
        initial.shape(),
        Appearance { reveal: 0.75, ..appearance },
        PaperFlip::new(1).frame(Duration::ZERO),
        ProgressFrame { completed: 0.4, activity: None },
        None,
    )?;
    let first = support::pending_frame(&mut driver, &wake)?;
    assert_eq!(driver.published_appearance().ok_or("published appearance absent")?.reveal, 0.25);
    let second = support::pending_frame(&mut driver, &wake)?;
    assert_eq!(driver.published_appearance().ok_or("published appearance absent")?.reveal, 0.75);
    drop((initial, first, second));
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// An unseen surface retains neither a request nor an admitted allocation.
#[test]
fn hidden_surface_updates_geometry_without_admitting_work() -> Result<(), Box<dyn Error>> {
    let (mut driver, wake, mut owner) = support::started()?;
    driver.set_visible(false)?;
    let shape = FrameShape::new(837, 837, 1.5)?;
    driver.request(
        shape,
        Appearance::demo(),
        PaperFlip::new(1).frame(Duration::ZERO),
        ProgressFrame { completed: 0.0, activity: None },
        None,
    )?;
    assert_eq!(driver.gate.shape(), shape);
    assert!(!driver.gate.visible());
    assert_eq!(driver.scope.snapshot().queued, 0);
    assert!(driver.pending.is_none());
    assert!(driver.active.is_none());
    driver.request_retirement()?;
    assert!(driver.poll_retirement()?);
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// Atomic memory rejection leaves the latest lightweight request available to its owner.
#[test]
fn saturated_initialization_preserves_pending_request_without_constructing_cache()
-> Result<(), Box<dyn Error>> {
    let (mut driver, wake, mut owner) = support::started()?;
    driver.set_visible(false)?;
    driver.set_visible(true)?;
    let client = &driver.client;
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let reservation_bytes = owner.limits().memory_bytes;
    let held = client.submit_blocking(
        &scope,
        JobSpec {
            id: driver.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes,
            replacement: None,
        },
        |_| Ok::<(), Infallible>(()),
    )?;
    let shape = FrameShape::new(558, 558, 1.0)?;
    driver.request(
        shape,
        Appearance::demo(),
        PaperFlip::new(1).frame(Duration::ZERO),
        ProgressFrame { completed: 0.25, activity: None },
        None,
    )?;
    let pending = driver.pending.as_ref().ok_or("Rejected request was lost")?;
    let request = pending.input.take()?;
    assert_eq!(request.shape, shape);
    assert_eq!(request.feedback.completed, 0.25);
    driver.pending.as_mut().ok_or("Pending absent")?.input = PreparationTransfer::new(request);
    assert!(driver.active.is_none());
    assert!(driver.renderer.is_none());
    driver.request_retirement()?;
    drop(held);
    drop(scope);
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// Busy admission preserves the existing cache transfer and the newest lightweight request.
#[test]
fn saturated_preparation_retains_existing_renderer_and_latest_request() -> Result<(), Box<dyn Error>>
{
    let (mut driver, wake, mut owner) = support::started()?;
    let retained = support::prepare(&mut driver, &wake, 0.0)?;
    let original = driver.renderer.as_ref().ok_or("Missing idle renderer")?.clone();
    let client = driver.client.clone();
    let held_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let already_reserved = driver.scope.snapshot().reserved_bytes;
    let available = owner
        .limits()
        .memory_bytes
        .checked_sub(already_reserved)
        .ok_or("Invalid reservation accounting")?;
    assert!(available > 0);
    let mut held = client.submit_blocking(
        &held_scope,
        JobSpec {
            id: driver.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: available,
            replacement: None,
        },
        |_| Ok::<(), Infallible>(()),
    )?;
    support::request(&mut driver, 0.25)?;
    support::request(&mut driver, 0.75)?;
    let pending = driver.pending.as_ref().ok_or("Busy admission lost the latest request")?;
    let identity = pending.identity;
    let request = pending.input.take()?;
    assert_eq!(request.feedback.completed, 0.75);
    driver.pending.as_mut().ok_or("Pending absent")?.input = PreparationTransfer::new(request);
    assert!(driver.active.is_none());
    assert!(!driver.closing);
    let renderer = original.take()?;
    assert!(matches!(
        driver.renderer.as_ref().ok_or("Busy admission lost renderer ownership")?.take(),
        Err(crate::render_transfer::TransferError::Consumed)
    ));
    driver.renderer = Some(PreparationTransfer::new(renderer));
    drop(support::completion(&mut held, &wake)?);
    let prepared = support::pending_frame(&mut driver, &wake)?;
    assert_eq!(prepared.identity(), identity);
    assert!(driver.pending.is_none());
    support::finish(&mut owner, &mut driver, &wake)?;
    assert!(!retained.pixels().is_empty());
    Ok(())
}

/// Real completed jobs restore their cache even when state or visibility rejects their pixels.
#[test]
fn stale_and_hidden_terminal_results_restore_cache_without_publication()
-> Result<(), Box<dyn Error>> {
    for hidden in [false, true] {
        let (mut driver, wake, mut owner) = support::started()?;
        let retained = support::prepare(&mut driver, &wake, 0.0)?;
        support::request(&mut driver, 0.5)?;
        let identity = driver
            .active
            .as_ref()
            .and_then(|active| active.identity)
            .ok_or("Missing admitted frame identity")?;
        support::terminal(&driver, &wake)?;
        if hidden {
            driver.set_visible(false)?;
        } else {
            driver.invalidate();
        }
        assert!(support::poll(&mut driver)?.is_none());
        assert!(driver.active.is_none());
        assert_ne!(driver.gate.state_generation(), identity.state_generation);
        let renderer = driver.renderer.as_ref().ok_or("Rejected result lost cache")?.take()?;
        driver.renderer = Some(PreparationTransfer::new(renderer));
        assert!(!driver.enriched());
        assert_eq!(retained.identity().surface, driver.gate.surface());
        support::finish(&mut owner, &mut driver, &wake)?;
    }
    Ok(())
}

/// Render retirement neither cancels unrelated work nor joins the shared execution owner.
#[test]
fn retirement_is_scope_local_and_preserves_unrelated_execution() -> Result<(), Box<dyn Error>> {
    let (mut driver, wake, mut owner) = support::started()?;
    let retained = support::prepare(&mut driver, &wake, 0.0)?;
    let client = driver.client.clone();
    let unrelated =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let latch = support::BlockingLatch::new();
    let worker = latch.clone();
    let mut blocked = client.submit_blocking(
        &unrelated,
        JobSpec {
            id: driver.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: 0,
            replacement: None,
        },
        move |_| worker.run(),
    )?;
    latch.wait_started()?;
    driver.request_retirement()?;
    assert!(driver.poll_retirement()?);
    driver.finish_retirement()?;
    assert!(driver.renderer.is_none());
    assert!(!unrelated.snapshot().cancellation_requested);
    assert_eq!(unrelated.snapshot().running, 1);
    let mut later = client.submit_async(
        &unrelated,
        JobSpec {
            id: driver.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::IoOrdinary,
            deadline: None,
            reservation_bytes: 0,
            replacement: None,
        },
        |_| async { Ok::<_, Infallible>(()) },
    )?;
    drop(support::completion(&mut later, &wake)?);
    latch.release()?;
    drop(support::completion(&mut blocked, &wake)?);
    assert_eq!(retained.pixels().len(), retained.shape().byte_len());
    assert!(retained.reserved_bytes() > 0);
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// A foreign notice cannot acknowledge the render result; the exact centrally drained notice does.
#[test]
fn consumer_leaves_foreign_metadata_and_terminal_ownership_untouched() -> Result<(), Box<dyn Error>>
{
    let (mut driver, wake, mut owner) = support::started()?;
    support::request(&mut driver, 0.0)?;
    support::terminal(&driver, &wake)?;
    let foreign = CompletionBatch {
        notifications: vec![sugarsubstitute_contracts::execution::CompletionNotification {
            scope_id: ScopeId::new(2)?,
            job_id: JobId::new(u64::MAX)?,
            kind: sugarsubstitute_contracts::execution::TerminalKind::Success,
            replacement: None,
        }],
        remaining: 0,
    };
    assert!(driver.consume(&foreign)?.is_none());
    assert!(driver.active.is_some());
    assert_eq!(foreign.notifications.len(), 1);
    support::poll(&mut driver)?;
    assert!(driver.renderer.is_some());
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// Global shutdown still observes actual worker stop while caller-retained pixels stay charged.
#[test]
fn shared_shutdown_retains_workers_until_blocking_work_actually_stops() -> Result<(), Box<dyn Error>>
{
    let (mut driver, wake, mut owner) = support::started()?;
    let retained = support::prepare(&mut driver, &wake, 0.0)?;
    let client = driver.client.clone();
    let unrelated =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let latch = support::BlockingLatch::new();
    let worker = latch.clone();
    let mut blocked = client.submit_blocking(
        &unrelated,
        JobSpec {
            id: driver.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: 0,
            replacement: None,
        },
        move |_| worker.run(),
    )?;
    latch.wait_started()?;
    driver.request_retirement()?;
    owner.request_shutdown(sugarsubstitute_contracts::execution::Moment::from_nanos(5_000_000_000));
    assert!(driver.poll_retirement()?);
    assert!(!owner.poll_shutdown()?.settled);
    assert!(owner.finish_shutdown().is_err());
    assert!(driver.renderer.is_some());
    assert!(driver.scope.snapshot().reserved_bytes >= retained.reserved_bytes());
    latch.release()?;
    drop(support::completion(&mut blocked, &wake)?);
    support::finish(&mut owner, &mut driver, &wake)?;
    assert!(driver.renderer.is_none());
    assert_eq!(retained.pixels().len(), retained.shape().byte_len());
    Ok(())
}

/// Real rejection retains one input credit; replacement and hiding release only their actual request.
#[test]
fn console_rejection_supersession_and_hide_preserve_exact_snapshot_custody()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::transcript::TranscriptError;
    let store = support::transcript(1)?;
    let held = [store.snapshot_tail(1)?, store.snapshot_tail(1)?];
    let (mut driver, wake, mut owner) = support::started()?;
    let memory = support::saturate(&driver, &owner)?;
    let input = || -> Result<_, Box<dyn Error>> {
        Ok((
            FrameShape::new(558, 558, 1.0)?,
            Appearance::demo(),
            AnimationFrame::bootstrap(),
            ProgressFrame { completed: 0.0, activity: None },
        ))
    };
    let (shape, appearance, character, feedback) = input()?;
    driver.request(shape, appearance, character, feedback, Some(store.snapshot_tail(1)?.into()))?;
    assert!(driver.active.is_none());
    assert!(driver.pending.is_some());
    let latest = store.snapshot_tail(1)?;
    assert!(matches!(store.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
    driver.request(shape, appearance, character, feedback, Some(latest.into()))?;
    let fourth = store.snapshot_tail(1)?;
    assert!(matches!(store.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
    driver.set_visible(false)?;
    assert!(driver.pending.is_none());
    assert!(store.snapshot_tail(1).is_ok());
    drop((fourth, held, memory));
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// A lower same-resource revision is rejected before current publication or pending ownership changes.
#[test]
fn console_regression_preserves_current_gate_and_pending_request() -> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::transcript::*;
    let mut store = support::transcript(1)?;
    let older = store.snapshot_tail(1)?;
    store.apply(&OutputBatch::new(
        store.resource(),
        OutputStream::Stdout,
        StreamSequence::new(1)?,
        b"actual revision\n",
        None,
        false,
    )?)?;
    let (mut driver, wake, mut owner) = support::started()?;
    let memory = support::saturate(&driver, &owner)?;
    let shape = FrameShape::new(558, 558, 1.0)?;
    let appearance = Appearance::demo();
    let character = AnimationFrame::bootstrap();
    let feedback = ProgressFrame { completed: 0.0, activity: None };
    driver.request(shape, appearance, character, feedback, Some(store.snapshot_tail(1)?.into()))?;
    let generation = driver.gate.state_generation();
    let identity = driver.pending.as_ref().ok_or("Pending absent")?.identity;
    assert!(driver.request(shape, appearance, character, feedback, Some(older.into())).is_err());
    assert_eq!(driver.gate.state_generation(), generation);
    assert_eq!(driver.pending.as_ref().ok_or("Pending replaced")?.identity, identity);
    let request = driver.pending.as_ref().ok_or("Pending absent")?.input.take()?;
    assert_eq!(request.console.as_ref().ok_or("Pin absent")?.revision(), store.revision());
    driver.pending.as_mut().ok_or("Pending absent")?.input = PreparationTransfer::new(request);
    drop(memory);
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// Actual completed old-resource and None-transition frames are rejected before new pixels publish.
#[test]
fn console_resource_and_none_transitions_invalidate_actual_terminal_frames()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::transcript::TranscriptError;
    let first = support::transcript(1)?;
    let next = support::transcript(2)?;
    let held = [first.snapshot_tail(1)?, first.snapshot_tail(1)?, first.snapshot_tail(1)?];
    let (mut driver, wake, mut owner) = support::started()?;
    let bootstrap = support::prepare(&mut driver, &wake, 0.0)?;
    let shape = FrameShape::new(558, 558, 1.0)?;
    let appearance = Appearance::demo();
    let character = AnimationFrame::bootstrap();
    let feedback = ProgressFrame { completed: 0.25, activity: None };
    driver.request(shape, appearance, character, feedback, Some(first.snapshot_tail(1)?.into()))?;
    support::terminal(&driver, &wake)?;
    assert!(matches!(first.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
    let old =
        driver.active.as_ref().and_then(|active| active.identity).ok_or("Old identity absent")?;
    driver.request(shape, appearance, character, feedback, Some(next.snapshot_tail(1)?.into()))?;
    assert_ne!(driver.gate.state_generation(), old.state_generation);
    assert!(support::poll(&mut driver)?.is_none());
    assert!(first.snapshot_tail(1).is_ok());
    support::terminal(&driver, &wake)?;
    let generation = driver.gate.state_generation();
    driver.request(shape, appearance, character, feedback, None)?;
    assert_ne!(driver.gate.state_generation(), generation);
    assert!(support::poll(&mut driver)?.is_none());
    let current = support::pending_frame(&mut driver, &wake)?;
    assert_eq!(current.identity().state_generation, driver.gate.state_generation());
    drop((bootstrap, current, held));
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}

/// Accepted queued cancellation/coalescing keeps original input until actual terminal observation.
#[test]
fn console_uninvoked_factory_keeps_pin_until_real_terminal_disposal() -> Result<(), Box<dyn Error>>
{
    use sugarsubstitute_contracts::{
        execution::{JobPhase, TerminalKind},
        transcript::TranscriptError,
    };
    for coalesce in [false, true] {
        let store = support::transcript(1)?;
        let held = [store.snapshot_tail(1)?, store.snapshot_tail(1)?, store.snapshot_tail(1)?];
        let (mut driver, wake, mut owner) = support::started()?;
        let bootstrap = support::prepare(&mut driver, &wake, 0.0)?;
        let scope =
            driver.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        // The production owner has exactly one interactive lane, also on single-CPU systems.
        let latch = support::BlockingLatch::new();
        let worker = latch.clone();
        let mut occupant = driver.client.submit_cpu(
            &scope,
            JobSpec {
                id: driver.jobs.allocate()?,
                operation: Operation::FramePrepare,
                class: JobClass::CpuInteractive,
                deadline: None,
                reservation_bytes: 0,
                replacement: Some(ReplacementKey {
                    surface: SurfaceId::new(2)?,
                    revision: Generation::new(1)?,
                }),
            },
            move |_| support::CpuHold(worker),
        )?;
        latch.wait_started()?;
        driver.request(
            FrameShape::new(558, 558, 1.0)?,
            Appearance::demo(),
            AnimationFrame::bootstrap(),
            ProgressFrame { completed: 0.0, activity: None },
            Some(store.snapshot_tail(1)?.into()),
        )?;
        assert_eq!(
            driver.active.as_ref().ok_or("Active absent")?.job.status().phase,
            JobPhase::Queued
        );
        let mut replacement = if coalesce {
            let mut key = driver
                .active
                .as_ref()
                .ok_or("Active absent")?
                .job
                .spec()
                .replacement
                .ok_or("Replacement key absent")?;
            key.revision = key.revision.next()?;
            let ready = support::BlockingLatch::new();
            ready.release()?;
            Some(driver.client.submit_cpu(
                &driver.scope,
                JobSpec {
                    id: driver.jobs.allocate()?,
                    operation: Operation::FramePrepare,
                    class: JobClass::CpuInteractive,
                    deadline: None,
                    reservation_bytes: 0,
                    replacement: Some(key),
                },
                move |_| support::CpuHold(ready),
            )?)
        } else {
            driver.request_retirement()?;
            None
        };
        assert!(matches!(store.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
        latch.release()?;
        drop(support::completion(&mut occupant, &wake)?);
        if let Some(job) = &mut replacement {
            drop(support::completion(job, &wake)?);
        }
        support::terminal(&driver, &wake)?;
        assert_eq!(
            driver.active.as_ref().ok_or("Active absent")?.job.status().phase,
            JobPhase::Terminal(if coalesce {
                TerminalKind::Coalesced
            } else {
                TerminalKind::Cancelled
            })
        );
        assert!(matches!(store.snapshot_tail(1), Err(TranscriptError::SnapshotCapacity)));
        let active = driver.active.as_mut().ok_or("Active absent")?;
        let input = active._request.as_ref().ok_or("Request guard absent")?.take()?;
        assert_eq!(
            input.console.as_ref().ok_or("Original input absent")?.resource(),
            store.resource()
        );
        active._request = Some(PreparationTransfer::new(input));
        assert!(support::poll(&mut driver)?.is_none());
        assert!(store.snapshot_tail(1).is_ok());
        drop((bootstrap, held, occupant, replacement, scope));
        support::finish(&mut owner, &mut driver, &wake)?;
    }
    Ok(())
}

#[path = "preparation_console_tests.rs"]
mod console;
