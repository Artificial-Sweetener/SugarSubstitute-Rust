// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Returning to a final while a live preview is on screen reuses its finished document.

use super::*;

fn blue(pixel: u32) -> bool {
    dominant(pixel, 0)
}

/// Offer a live preview of `color` and acknowledge frames until it is on screen over the current
/// final. The fixture's own step reads a document's pending evidence, which a preview frame
/// keeps in its candidate, so this loop acknowledges frames by target.
fn show_preview(fixture: &mut Fixture, color: [u8; 3]) -> Result<(), Box<dyn Error>> {
    let deadline = fixture.session.preview_now().checked_add(Duration::from_secs(30))?;
    let mut preview = Some(fixture.output(color)?);
    assert!(fixture.session.offer_preview(&mut preview, deadline)?.token().is_some());
    for _ in 0..1000 {
        if fixture.session.preview_shown() {
            return Ok(());
        }
        let observed = fixture.wake.revision();
        fixture.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        fixture.session.poll(Duration::ZERO)?;
        if let Some(target) =
            fixture.session.frame().filter(|frame| frame.pending).map(|frame| frame.target)
        {
            fixture.session.did_present_cpu(target)?;
            continue;
        }
        let progress = fixture.owner.poll_shutdown()?;
        if progress.running + progress.queued > 0 {
            fixture.wake.wait_after(observed);
        } else {
            fixture.clock.0.fetch_add(100_000_000, Ordering::SeqCst);
        }
    }
    Err("The live preview never reached the screen".into())
}

/// The next frame shown for the current selection, failing if its source is loaded again.
fn shown_without_loading(fixture: &mut Fixture) -> Result<Shown, Box<dyn Error>> {
    let selection = fixture.session.selection;
    let shown = fixture.next_shown(|session| {
        (session.loading.is_none() && session.pending_open.is_none())
            .then_some(())
            .ok_or("A retained output was loaded again")
    })?;
    assert_eq!(shown.evidence.source_revision, selection);
    Ok(shown)
}

#[test]
fn a_final_chosen_while_a_preview_is_shown_returns_without_loading() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    fixture.show([0, 0, 255], "b")?;
    // Another stage's live preview covers final b, as a running workflow's previews do.
    show_preview(&mut fixture, [0, 255, 0])?;

    // Choosing a parked final withdraws the preview, as the host's selection does first.
    fixture.session.withdraw_preview()?;
    assert!(fixture.session.reselect_retained(&key("a"))?);
    let shown = shown_without_loading(&mut fixture)?;
    assert_eq!(shown.evidence.tier, BackingTier::Detail);
    assert!(red(shown.pixel), "{:x}", shown.pixel);
    fixture.settle()?;
    assert!(fixture.session.retained.contains(&key("b")), "The covered final was not kept");

    // The final the preview covered is reused as well.
    assert!(fixture.session.reselect_retained(&key("b"))?);
    let shown = shown_without_loading(&mut fixture)?;
    assert!(blue(shown.pixel), "{:x}", shown.pixel);
    fixture.settle()?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn the_final_under_a_shown_preview_returns_without_loading() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    show_preview(&mut fixture, [0, 255, 0])?;

    fixture.session.withdraw_preview()?;
    assert!(fixture.session.reselect_retained(&key("a"))?);
    let shown = shown_without_loading(&mut fixture)?;
    assert!(red(shown.pixel), "{:x}", shown.pixel);
    fixture.settle()?;
    assert_eq!(fixture.session.document.retention, Some(key("a")));
    assert!(!fixture.session.preview_pending(), "Preview custody never settled");
    let _directory = fixture.finish()?;
    Ok(())
}
