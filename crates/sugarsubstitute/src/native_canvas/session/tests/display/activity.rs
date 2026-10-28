// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native activity is coalesced demand, never a new authored image or a permanent repaint timer.
use super::presentation::{consume_present, presenting};
use super::*;

#[test]
fn pointer_demand_during_native_presentation_survives_the_earlier_receipt()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, release, target) = presenting(false)?;
    let view = fixture.session.document.tools.navigation().viewport();
    let document =
        fixture.session.document.scene.as_ref().ok_or("Missing document")?.document.snapshot();
    let identity = (document.identity(), document.revision());
    assert!(!fixture.session.expose_pending);
    for _ in 0..100 {
        fixture.session.expose();
    }
    assert!(fixture.session.expose_pending);
    assert_eq!(fixture.session.document.tools.navigation().viewport(), view);
    assert!(fixture.session.rendering.is_none());
    assert!(
        fixture.session.next_deadline().is_none(),
        "Running presentation supplies its own wake"
    );
    drop(release);
    consume_present(&mut fixture.session, &fixture.wake)?;
    let frame = fixture.session.take_gpu_presentation().ok_or("Missing native receipt")?;
    assert_eq!(frame.target, target);
    fixture.session.did_present_gpu(target)?;
    assert!(fixture.session.expose_pending, "Earlier queue acceptance consumed newer activity");
    let actual =
        fixture.session.document.scene.as_ref().ok_or("Missing document")?.document.snapshot();
    assert_eq!((actual.identity(), actual.revision()), identity);
    Ok(())
}

#[test]
fn native_acceptance_without_new_activity_has_no_repaint_timer() -> Result<(), Box<dyn Error>> {
    let (mut fixture, release, target) = presenting(false)?;
    drop(release);
    consume_present(&mut fixture.session, &fixture.wake)?;
    fixture.session.take_gpu_presentation().ok_or("Missing native receipt")?;
    fixture.session.did_present_gpu(target)?;
    assert!(!fixture.session.expose_pending);
    assert!(fixture.session.next_deadline().is_none());
    fixture.clock.0.store(5_000_000_000, Ordering::SeqCst);
    assert!(fixture.session.next_deadline().is_none());
    assert!(fixture.session.rendering.is_none());
    Ok(())
}

#[test]
fn unavailable_viewport_suspends_native_retries_and_restore_marks_one_demand()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, release, target) = presenting(false)?;
    drop(release);
    consume_present(&mut fixture.session, &fixture.wake)?;
    fixture.session.take_gpu_presentation().ok_or("Missing native receipt")?;
    fixture.session.did_present_gpu(target)?;
    fixture.session.expose();
    fixture.session.input(Input::Drawable(false))?;
    assert!(fixture.session.expose_pending);
    assert!(fixture.session.next_deadline().is_none());
    fixture.session.submit_display()?;
    assert!(fixture.session.displaying.is_none());
    fixture.session.input(Input::Drawable(true))?;
    assert!(fixture.session.expose_pending);
    assert!(fixture.session.next_deadline().is_some());
    assert!(fixture.session.rendering.is_none());
    Ok(())
}
