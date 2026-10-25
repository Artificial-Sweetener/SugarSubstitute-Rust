// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real render admission, stale-result rejection, cache retention and shutdown.
use super::*;
mod support;
use support::Fixture;

#[test]
fn edited_page_rejects_completed_old_pixels_and_coalesces_pending_input() -> Result {
    let mut f = Fixture::new()?;
    let original = f.frame()?;
    let inputs = f.driver.inputs.as_ref().ok_or("Inputs absent")?.clone();
    f.driver.invalidate()?;
    assert!(f.poll()?.is_none());
    f.terminal()?;
    let stale = f
        .driver
        .active
        .as_ref()
        .and_then(|active| active.request.as_ref())
        .ok_or("Old render absent")?
        .identity;
    // Change native interaction after the worker has finished but before terminal delivery.
    f.view.interaction.hovered = Some(Action::Primary.id());
    f.driver.invalidate()?;
    assert!(f.poll()?.is_none());
    assert!(f.driver.inputs.as_ref().ok_or("Inputs absent")?.same_reservation(&inputs));
    f.view.interaction.pressed = Some(Action::Primary.id());
    f.driver.invalidate()?;
    // Do not drain the active completion: repeated updates must fit one pending-input credit.
    let batch = CompletionBatch { notifications: Vec::new(), remaining: 0 };
    f.driver.advance(&f.flow, f.view, &batch)?;
    f.view.interaction.hovered = Some(Action::Close.id());
    f.driver.invalidate()?;
    f.driver.advance(&f.flow, f.view, &batch)?;
    let latest = f.driver.pending.as_ref().ok_or("Latest request absent")?.identity;
    let prepared = f.frame()?;
    assert_eq!(prepared.frame.identity(), latest);
    assert_ne!(prepared.frame.identity(), stale);
    assert_ne!(prepared.frame.pixels(), original.frame.pixels());
    drop((prepared, original, inputs));
    f.finish()?;
    assert_eq!(f.driver.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn saturated_admission_preserves_fonts_and_retries_only_latest_page() -> Result {
    use sugarsubstitute_contracts::execution::{ScopeId, ScopeSpec};
    let mut f = Fixture::new()?;
    let original = f.frame()?;
    let inputs = f.driver.inputs.as_ref().ok_or("Inputs absent")?.clone();
    let scope =
        f.driver.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let available = f.owner.limits().memory_bytes - f.driver.scope.snapshot().reserved_bytes;
    let mut held = f.driver.client.submit_blocking(
        &scope,
        JobSpec {
            id: f.driver.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: available,
            replacement: None,
        },
        |_| Ok::<(), io::Error>(()),
    )?;
    f.driver.invalidate()?;
    assert!(f.poll()?.is_none());
    assert!(f.driver.active.is_none());
    f.view.interaction.hovered = Some(Action::Close.id());
    f.driver.invalidate()?;
    assert!(f.poll()?.is_none());
    let latest = f.driver.pending.as_ref().ok_or("Pending absent")?.identity;
    assert!(f.driver.inputs.as_ref().ok_or("Inputs absent")?.same_reservation(&inputs));
    drop(f.wake.observe(|| Ok(held.try_take_completion()))?);
    let frame = f.frame()?;
    assert_eq!(frame.frame.identity(), latest);
    drop((frame, original, inputs, held, scope));
    f.finish()?;
    assert_eq!(f.driver.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn invalid_scene_returns_fonts_and_next_valid_view_renders() -> Result {
    let mut f = Fixture::new()?;
    let original = f.frame()?;
    let inputs = f.driver.inputs.as_ref().ok_or("Inputs absent")?.clone();
    f.view.scroll = f32::NAN;
    f.driver.invalidate()?;
    let cause = f.wake.clone().observe(|| match f.poll() {
        Err(error) => Ok(Some(error)),
        Ok(None) => Ok(None),
        Ok(Some(_)) => Err("Invalid view was published".into()),
    })?;
    assert_eq!(cause.to_string(), "Invalid installer viewport");
    assert!(f.driver.inputs.as_ref().ok_or("Inputs absent")?.same_reservation(&inputs));
    f.view.scroll = 0.0;
    f.driver.invalidate()?;
    let recovered = f.frame()?;
    assert_eq!(original.frame.pixels(), recovered.frame.pixels());
    drop((original, recovered, inputs));
    f.finish()
}

#[test]
fn hidden_and_closing_surfaces_never_publish_completed_frames() -> Result {
    let mut f = Fixture::new()?;
    f.driver.set_visible(false)?;
    f.view.surface.width = 0;
    f.view.surface.height = 0;
    assert!(f.poll()?.is_none());
    assert!(f.driver.active.is_none());
    assert_eq!(f.driver.scope.snapshot().reserved_bytes, 0);
    f.view.surface.width = 720;
    f.view.surface.height = 560;
    f.driver.set_visible(true)?;
    let frame = f.frame()?;
    f.driver.invalidate()?;
    assert!(f.poll()?.is_none());
    f.terminal()?;
    f.driver.set_visible(false)?;
    assert!(f.poll()?.is_none());
    f.driver.set_visible(true)?;
    assert!(f.poll()?.is_none());
    f.driver.close()?;
    f.driver.close()?;
    f.view.surface.width = 0;
    f.view.surface.height = 0;
    assert!(f.driver.finish().is_err());
    f.finish()?;
    // Published pixels keep their original custody after workers have actually joined.
    assert!(!frame.frame.pixels().is_empty());
    assert!(f.driver.scope.snapshot().reserved_bytes > 0);
    drop(frame);
    assert_eq!(f.driver.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
