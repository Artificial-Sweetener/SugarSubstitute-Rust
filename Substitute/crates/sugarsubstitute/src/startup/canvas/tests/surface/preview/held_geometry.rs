// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Geometry held for a preview frame is always answered, whatever happens to that preview.
use super::*;

/// Show `preview:1` over the fixture's committed attachment.
fn show_first_preview(fixture: &mut Fixture) -> Result<FrameStamp, Box<dyn Error>> {
    watch(fixture, 100, true)?;
    transfer(fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(fixture, "presented")?;
    Ok(target)
}

#[test]
fn geometry_held_for_a_refit_is_answered_when_a_newer_transfer_is_cancelled()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    show_first_preview(&mut fixture)?;
    fixture.geometry(324, 60, 30)?;
    begin(&mut fixture, 400, "preview:2", 3)?;
    fixture.send(
        401,
        "surface.preview.withdraw",
        json!({"token":"preview:2","leaseRevision":400}),
    )?;
    fixture.take(401, "applied")?;
    settle_geometry(&mut fixture, 324)?;
    drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn geometry_is_answered_after_the_shown_preview_is_withdrawn_over_an_empty_canvas()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(3, "surface.commit", json!({}))?;
    fixture.canvas.hidden(fixture.identity)?;
    fixture.take(3, "applied")?;
    show_first_preview(&mut fixture)?;
    fixture.geometry(324, 60, 30)?;
    fixture.send(
        325,
        "surface.preview.withdraw",
        json!({"token":"preview:1","leaseRevision":200}),
    )?;
    let mut answered = std::collections::BTreeSet::new();
    let mut hidden = false;
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        while let Some(command) = fixture.canvas.take_command() {
            if let DesktopStartupCommand::SendSurface(reply) = command {
                let value: Value = serde_json::from_slice(reply.payload())?;
                answered.insert(value["requestId"].as_u64().ok_or("Reply without id")?);
            }
        }
        if answered.contains(&324) && answered.contains(&325) {
            drain(&mut fixture)?;
            return fixture.finish();
        }
        match fixture.canvas.presentation() {
            MainCanvasPresentation::Frame(frame) if frame.content.pending() => {
                fixture.canvas.presented(frame.content.target())?;
            }
            MainCanvasPresentation::Hide if !hidden => {
                hidden = true;
                fixture.canvas.hidden(fixture.identity)?;
            }
            _ => fixture.wait_preview_progress(observed)?,
        }
    }
    Err(format!("Requests left unanswered after withdrawal; answered {answered:?}").into())
}

#[test]
fn resizing_while_previews_keep_arriving_answers_every_geometry() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    show_first_preview(&mut fixture)?;
    for step in 0..4_u64 {
        let id = 500 + step * 10;
        fixture.geometry(id, 60 + 10 * u32::try_from(step)?, 30 + 5 * u32::try_from(step)?)?;
        transfer(&mut fixture, id + 1, &format!("preview:{}", step + 2), JPEG)?;
        settle_geometry(&mut fixture, id)?;
    }
    drain(&mut fixture)?;
    fixture.finish()
}
