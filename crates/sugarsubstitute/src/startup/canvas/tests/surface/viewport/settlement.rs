// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Every viewport request reaches one terminal reply, whatever keeps its camera from settling:
//! a display that no longer shows the current source draws again, a camera that cannot be
//! presented is answered `unavailable` once its settle budget ends, and a newer destination
//! answers it `stale`. A request answered that way never wedges the requests after it.

use super::fixture::Execution;
use super::*;
use crate::canvas_surface_diagnostic::{Stage, Wait};
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

/// A presenter that never acknowledges the zoomed frame cannot settle the camera; the request is
/// answered `unavailable` once its settle budget ends, the expiry records what it waited for,
/// and the next request settles once the frame is acknowledged.
#[test]
fn a_camera_whose_frame_is_never_acknowledged_is_answered_and_does_not_wedge_the_next()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let lines = DiagnosticLines::default();
    let capture = lines.observe();
    fixture.viewport(5, json!({"type":"zoom","factor":2.0}))?;
    assert_eq!(answer(&mut fixture, 5, false)?["status"], "unavailable");
    drop(capture);
    let records = lines.requests(5)?;
    let expired = records
        .iter()
        .find(|record| record.stage == Stage::Expired)
        .ok_or("The expiry was not recorded")?;
    assert_eq!(expired.wait, Wait::Presenting);
    assert!(
        records
            .iter()
            .any(|record| record.stage == Stage::Waiting && record.wait == Wait::Presenting),
        "the wait reason is recorded before the expiry: {records:?}"
    );
    fixture.viewport(6, json!({"type":"read"}))?;
    let reply = answer(&mut fixture, 6, true)?;
    snapshot(&reply, 6, "custom", 0.5, [320, 160]);
    fixture.finish()
}

/// A canvas that is not drawable cannot present the zoomed camera; the request is answered
/// `unavailable` once its settle budget ends, and the camera settles once it is drawable again.
#[test]
fn a_camera_on_a_canvas_that_cannot_draw_is_answered_and_does_not_wedge_the_next()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    Lifetime::input(session(&mut fixture)?, Input::Drawable(false))?;
    let lines = DiagnosticLines::default();
    let capture = lines.observe();
    fixture.viewport(5, json!({"type":"zoom","factor":2.0}))?;
    assert_eq!(answer(&mut fixture, 5, true)?["status"], "unavailable");
    drop(capture);
    let waits: Vec<_> =
        lines.requests(5)?.iter().map(|record| (record.stage, record.wait)).collect();
    assert!(waits.contains(&(Stage::Waiting, Wait::Hidden)), "{waits:?}");
    assert!(waits.contains(&(Stage::Expired, Wait::Hidden)), "{waits:?}");
    Lifetime::input(session(&mut fixture)?, Input::Drawable(true))?;
    fixture.viewport(6, json!({"type":"read"}))?;
    let reply = answer(&mut fixture, 6, true)?;
    snapshot(&reply, 6, "custom", 0.5, [320, 160]);
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
