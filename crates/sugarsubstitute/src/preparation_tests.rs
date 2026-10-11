// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise preparation admission and hidden-state behavior through the real execution port.

use super::*;
use std::convert::Infallible;
use sugarsubstitute_contracts::execution::ExecutionError;
use sugarsubstitute_splash::PaperFlip;

#[path = "preparation_test_support.rs"]
mod support;

/// Accept terminal notifications in tests that own no native window or event loop.
struct SilentWake;
impl CompletionWake for SilentWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}

/// An unseen surface retains neither a request nor an admitted allocation.
#[test]
fn hidden_surface_updates_geometry_without_starting_execution() -> Result<(), Box<dyn Error>> {
    let mut driver = PreparationDriver::new("en")?;
    let shape = FrameShape::new(837, 837, 1.5)?;
    driver.request(
        shape,
        Appearance::demo(),
        PaperFlip::new(1).frame(Duration::ZERO),
        ProgressFrame { completed: 0.0, activity: None },
    )?;
    assert_eq!(driver.gate.shape(), shape);
    assert!(!driver.gate.visible());
    assert!(driver.owner.is_none());
    assert!(driver.pending.is_none());
    assert!(driver.active.is_none());
    driver.request_shutdown()?;
    assert!(driver.poll_shutdown()?);
    driver.finish_shutdown()?;
    Ok(())
}

/// Atomic memory rejection leaves the latest lightweight request available to its owner.
#[test]
fn saturated_initialization_preserves_pending_request_without_constructing_cache()
-> Result<(), Box<dyn Error>> {
    let mut driver = PreparationDriver::new("en")?;
    driver.start(Arc::new(SilentWake))?;
    driver.set_visible(true)?;
    let client = driver.client.as_ref().ok_or("Missing test client")?;
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let reservation_bytes =
        driver.owner.as_ref().ok_or("Missing test owner")?.limits().memory_bytes;
    let held = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
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
    driver.request_shutdown()?;
    drop(held);
    drop(scope);
    // This test owns no UI resources. Executor Drop observes worker disposal and joins.
    drop(driver);
    Ok(())
}

/// Busy admission preserves the existing cache transfer and the newest lightweight request.
#[test]
fn saturated_preparation_retains_existing_renderer_and_latest_request() -> Result<(), Box<dyn Error>>
{
    let (mut driver, wake) = support::started()?;
    let retained = support::prepare(&mut driver, &wake, 0.0)?;
    let original = driver.renderer.as_ref().ok_or("Missing idle renderer")?.clone();
    let client = driver.client.as_ref().ok_or("Missing execution client")?.clone();
    let held_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let already_reserved = driver.scope.as_ref().ok_or("Missing scope")?.snapshot().reserved_bytes;
    let available = driver
        .owner
        .as_ref()
        .ok_or("Missing owner")?
        .limits()
        .memory_bytes
        .checked_sub(already_reserved)
        .ok_or("Invalid reservation accounting")?;
    assert!(available > 0);
    let revision = driver.allocate_identity()?;
    let mut held = client.submit_blocking(
        &held_scope,
        JobSpec {
            id: JobId::new(revision.get())?,
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
    support::settle(&mut driver, &wake)?;
    driver.finish_shutdown()?;
    assert!(!retained.pixels().is_empty());
    Ok(())
}

/// Real completed jobs restore their cache even when state or visibility rejects their pixels.
#[test]
fn stale_and_hidden_terminal_results_restore_cache_without_publication()
-> Result<(), Box<dyn Error>> {
    for hidden in [false, true] {
        let (mut driver, wake) = support::started()?;
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
        assert!(driver.poll(NonZeroUsize::MIN)?.is_none());
        assert!(driver.active.is_none());
        assert_ne!(driver.gate.state_generation(), identity.state_generation);
        let renderer = driver.renderer.as_ref().ok_or("Rejected result lost cache")?.take()?;
        driver.renderer = Some(RendererTransfer::new(renderer));
        assert!(!driver.enriched());
        assert_eq!(retained.identity().surface, driver.gate.surface());
        support::settle(&mut driver, &wake)?;
        driver.finish_shutdown()?;
    }
    Ok(())
}

/// Cancellation does not impersonate stop, and retained frame charges do not block real settlement.
#[test]
fn shutdown_observes_actual_work_stop_while_prepared_frame_memory_remains_live()
-> Result<(), Box<dyn Error>> {
    let (mut driver, wake) = support::started()?;
    let retained = support::prepare(&mut driver, &wake, 0.0)?;
    let observer_scope = driver.scope.as_ref().ok_or("Missing scope")?;
    assert!(observer_scope.snapshot().reserved_bytes >= retained.reserved_bytes());
    let client = driver.client.as_ref().ok_or("Missing client")?.clone();
    let blocked_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let revision = driver.allocate_identity()?;
    let latch = support::BlockingLatch::new();
    let worker_latch = latch.clone();
    let mut blocked = client.submit_blocking(
        &blocked_scope,
        JobSpec {
            id: JobId::new(revision.get())?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: 0,
            replacement: None,
        },
        move |_| worker_latch.run(),
    )?;
    latch.wait_started()?;
    driver.request_shutdown()?;
    assert!(!driver.poll_shutdown()?);
    assert!(driver.finish_shutdown().is_err());
    assert!(driver.owner.is_some());
    assert!(driver.renderer.is_some());
    latch.release()?;
    drop(support::completion(&mut blocked, &wake)?);
    support::settle(&mut driver, &wake)?;
    let progress = driver.owner.as_ref().ok_or("Missing owner")?.poll_shutdown()?;
    assert!(progress.settled && progress.workers.async_finished && progress.workers.cpu_finished);
    assert!(driver.scope.as_ref().ok_or("Missing scope")?.snapshot().reserved_bytes > 0);
    driver.finish_shutdown()?;
    assert!(driver.owner.is_none() && driver.client.is_none() && driver.renderer.is_none());
    assert!(retained.reserved_bytes() > 0);
    assert_eq!(retained.pixels().len(), retained.shape().byte_len());
    Ok(())
}
