// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Every viewport request reaches one terminal reply, and only an observed event decides which:
//! a display that no longer shows the current source draws again, a camera that cannot yet be
//! presented waits however long that takes and applies once it can, a newer destination answers it
//! `stale`, and the attachment's release answers it `unavailable`. No elapsed time answers it.

use super::fixture::Execution;
use super::*;
use crate::canvas_surface_diagnostic::{Stage, Status, Wait};
use crate::startup::canvas::tests::surface::capture::DiagnosticLines;
use sugarsubstitute_desktop::canvas::{Input, Lifetime, MainCanvasGesture, MainCanvasInput};

/// Pump until request `id` is answered, returning its reply. `acknowledge` presents each offered
/// frame as the desktop does; without it, frames stay unacknowledged as from a stalled
/// presenter. Injected time advances only while no job runs. A request still held once no work
/// or deadline remains has no answer coming and fails.
fn answer(fixture: &mut Fixture, id: u64, acknowledge: bool) -> Result<Value, Box<dyn Error>> {
    for _ in 0..1_000 {
        while let Some(command) = fixture.canvas.take_command() {
            if let DesktopStartupCommand::SendSurface(reply) = command {
                let value: Value = serde_json::from_slice(reply.payload())?;
                if value["requestId"] == id {
                    return Ok(value);
                }
            }
        }
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if acknowledge
            && let Some(target) = prepared_frame(&fixture.canvas)
                .filter(|frame| frame.content.pending())
                .map(|frame| frame.content.target())
        {
            fixture.canvas.presented(target)?;
            continue;
        }
        match fixture.execution()? {
            Execution::Idle if fixture.canvas.next_deadline().is_none() => {
                let Some(DesktopStartupCommand::SendSurface(reply)) = fixture.canvas.take_command()
                else {
                    return Err(format!("Viewport request {id} was held without an answer").into());
                };
                let value: Value = serde_json::from_slice(reply.payload())?;
                if value["requestId"] == id {
                    return Ok(value);
                }
            }
            Execution::Idle | Execution::Delivering => fixture.advance(100),
            Execution::Running => fixture.wake.wait_after(observed),
        }
    }
    Err(format!("Viewport request {id} was never answered").into())
}

fn session(fixture: &mut Fixture) -> Result<&mut Session, Box<dyn Error>> {
    fixture.canvas.session.as_mut().ok_or_else(|| "The canvas session is absent".into())
}

/// A presenter acknowledgement the session no longer holds leaves a display that proves no
/// camera; a read draws again and settles instead of waiting forever.
#[test]
fn a_read_settles_after_the_shown_frame_lost_its_acknowledgement() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let fitted = fixture.commit(4)?;
    // Finish refinement first, so only the read itself can ask for another frame.
    fixture.settle()?;
    session(&mut fixture)?.lose_native_acknowledgement();
    fixture.viewport(5, json!({"type":"read"}))?;
    let reply = answer(&mut fixture, 5, true)?;
    snapshot(&reply, 5, "fit", 0.25, [320, 160]);
    assert_eq!(fixture.accepted()?.viewport, fitted.viewport);
    fixture.viewport(6, json!({"type":"zoom","factor":2.0}))?;
    assert_eq!(answer(&mut fixture, 6, true)?["status"], "applied");
    fixture.finish()
}

/// A display adopted from another document, as a document swap leaves until a frame of the
/// incoming document lands, is drawn again for a read on the current document.
#[test]
fn a_read_settles_after_the_display_described_another_document() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.settle()?;
    session(&mut fixture)?.adopt_foreign_display();
    let lines = DiagnosticLines::default();
    let capture = lines.observe();
    fixture.viewport(5, json!({"type":"read"}))?;
    let reply = answer(&mut fixture, 5, true)?;
    drop(capture);
    snapshot(&reply, 5, "fit", 0.25, [320, 160]);
    let stages: Vec<_> = lines.requests(5)?.iter().map(|record| record.stage).collect();
    assert_eq!(stages.first(), Some(&Stage::Admitted));
    assert_eq!(stages.last(), Some(&Stage::Replied));
    fixture.finish()
}

/// Advance injected time by `seconds` while the canvas keeps pumping, requiring that none of the
/// requests `ids` is answered after any second.
fn stay_pending(fixture: &mut Fixture, ids: &[u64], seconds: u64) -> Result<(), Box<dyn Error>> {
    for _ in 0..seconds {
        fixture.advance(1_000);
        fixture.pump()?;
        while let Some(command) = fixture.canvas.take_command() {
            if let DesktopStartupCommand::SendSurface(reply) = command {
                let value: Value = serde_json::from_slice(reply.payload())?;
                assert!(
                    !ids.iter().any(|id| value["requestId"] == *id),
                    "a request was answered early: {value}"
                );
            }
        }
    }
    Ok(())
}

/// A presenter that has not yet acknowledged the zoomed frame leaves the request pending, past
/// what the retired ten second budget allowed. The wait is recorded, never judged a failure, and the acknowledgement settles the
/// very request that waited.
#[test]
fn a_camera_whose_frame_is_acknowledged_late_waits_and_then_applies() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let lines = DiagnosticLines::default();
    let capture = lines.observe();
    fixture.viewport(5, json!({"type":"zoom","factor":2.0}))?;
    // Well past the retired ten second settle budget without the presenter's acknowledgement.
    stay_pending(&mut fixture, &[5], 25)?;
    assert_eq!(answer(&mut fixture, 5, true)?["status"], "applied");
    drop(capture);
    let records = lines.requests(5)?;
    assert!(
        records
            .iter()
            .any(|record| record.stage == Stage::Waiting && record.wait == Wait::Presenting),
        "the wait reason is recorded: {records:?}"
    );
    assert!(
        records.iter().all(|record| record.status != Status::Unavailable),
        "no unavailable verdict was recorded: {records:?}"
    );
    fixture.viewport(6, json!({"type":"read"}))?;
    let reply = answer(&mut fixture, 6, true)?;
    snapshot(&reply, 6, "custom", 0.5, [320, 160]);
    fixture.finish()
}

/// A canvas that is not drawable cannot present the zoomed camera. The request waits, hidden, for
/// as long as that lasts, and settles itself once the canvas is drawable again.
#[test]
fn a_camera_on_a_canvas_that_cannot_draw_waits_and_applies_once_it_can()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    Lifetime::input(session(&mut fixture)?, Input::Drawable(false))?;
    let lines = DiagnosticLines::default();
    let capture = lines.observe();
    fixture.viewport(5, json!({"type":"zoom","factor":2.0}))?;
    stay_pending(&mut fixture, &[5], 25)?;
    Lifetime::input(session(&mut fixture)?, Input::Drawable(true))?;
    assert_eq!(answer(&mut fixture, 5, true)?["status"], "applied");
    drop(capture);
    let waits: Vec<_> =
        lines.requests(5)?.iter().map(|record| (record.stage, record.wait)).collect();
    assert!(waits.contains(&(Stage::Waiting, Wait::Hidden)), "{waits:?}");
    fixture.viewport(6, json!({"type":"read"}))?;
    let reply = answer(&mut fixture, 6, true)?;
    snapshot(&reply, 6, "custom", 0.5, [320, 160]);
    fixture.finish()
}

/// The page releasing the attachment is an observed event that ends the wait: the pending request
/// is answered `unavailable`, ahead of the release itself.
#[test]
fn a_pending_camera_is_answered_when_its_attachment_is_released() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    Lifetime::input(session(&mut fixture)?, Input::Drawable(false))?;
    fixture.viewport(5, json!({"type":"zoom","factor":2.0}))?;
    stay_pending(&mut fixture, &[5], 25)?;
    fixture.send(6, "surface.release", json!({}))?;
    assert_eq!(answer(&mut fixture, 5, false)?["status"], "unavailable");
    assert_eq!(answer(&mut fixture, 6, false)?["status"], "applied");
    fixture.finish()
}

/// A wheel gesture that moves the camera elsewhere while a command waits replaces its
/// destination; the command is answered `stale` and a later read settles at the new camera.
#[test]
fn a_camera_replaced_by_a_newer_gesture_is_answered_stale() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"zoom","factor":2.0}))?;
    let identity = fixture.identity;
    let wheel = MainCanvasInput { identity, gesture: MainCanvasGesture::Wheel(-1.0) };
    fixture.canvas.input(wheel)?;
    assert_eq!(answer(&mut fixture, 5, true)?["status"], "stale");
    fixture.viewport(6, json!({"type":"read"}))?;
    assert_eq!(answer(&mut fixture, 6, true)?["status"], "applied");
    fixture.finish()
}

/// The attachment a page stages while a camera request on the committed attachment still waits.
fn other_attachment() -> Value {
    json!({
        "sessionId":"canvas:other", "viewId":"view:one", "windowId":"window:one",
        "attachmentId":"attachment:other", "generation":2
    })
}

/// A candidate arriving while a camera request waits is early, not unavailable: it is held behind
/// that request, neither answered nor failed however long the wait, and prepared once it ends.
#[test]
fn a_candidate_prepared_behind_a_waiting_camera_is_held_and_then_prepared()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"zoom","factor":2.0}))?;
    stay_pending(&mut fixture, &[5], 5)?;
    fixture.send(6, "surface.prepare", json!({"address":other_attachment()}))?;
    stay_pending(&mut fixture, &[5, 6], 25)?;
    assert_eq!(answer(&mut fixture, 5, true)?["status"], "applied");
    assert_eq!(answer(&mut fixture, 6, true)?["status"], "ready");
    fixture.send(7, "surface.release", json!({"address":other_attachment()}))?;
    assert_eq!(answer(&mut fixture, 7, true)?["status"], "applied");
    fixture.finish()
}
