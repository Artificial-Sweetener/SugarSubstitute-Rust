// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise preparation admission and hidden-state behavior through the real execution port.

use super::*;
use std::{convert::Infallible, time::Duration};
use sugarsubstitute_contracts::execution::{JobId, ScopeId, ScopeSpec};
use sugarsubstitute_splash::PaperFlip;

#[path = "preparation_test_support.rs"]
mod support;

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
    )?;
    let pending = driver.pending.as_ref().ok_or("Rejected request was lost")?;
    assert_eq!(pending.shape, shape);
    assert_eq!(pending.feedback.completed, 0.25);
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
    let pending = *driver.pending.as_ref().ok_or("Busy admission lost the latest request")?;
    assert_eq!(pending.feedback.completed, 0.75);
    assert!(driver.active.is_none());
    assert!(!driver.closing);
    let renderer = original.take()?;
    assert!(matches!(
        driver.renderer.as_ref().ok_or("Busy admission lost renderer ownership")?.take(),
        Err(crate::render_transfer::TransferError::Consumed)
    ));
    driver.renderer = Some(RendererTransfer::new(renderer));
    drop(support::completion(&mut held, &wake)?);
    let prepared = support::pending_frame(&mut driver, &wake)?;
    assert_eq!(prepared.identity(), pending.identity);
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
        driver.renderer = Some(RendererTransfer::new(renderer));
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
