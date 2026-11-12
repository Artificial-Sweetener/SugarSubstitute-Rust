// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A frame acknowledgement the canvas cannot accept never ends the application: stale, foreign and
//! hide-pending acknowledgements change nothing, a frame that cannot be accepted is dropped while the
//! last accepted frame stays shown, and only unclassified custody corruption stays fatal.

use super::viewport::fixture::Fixture;
use super::*;
use crate::native_canvas::session::{Ignored, Unaccepted};
use sugarsubstitute_canvas::FrameStamp;

/// A committed attachment whose first frame the native adapter accepted, with that frame.
fn committed() -> Result<(Fixture, FrameStamp), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.send(4, "surface.commit", json!({}))?;
    let first = fixture.pending()?;
    fixture.canvas.presented(first)?;
    fixture.take(4, "applied")?;
    Ok((fixture, first))
}

/// Resize the viewport so a new frame awaits acknowledgement, and return that frame.
fn next_frame(fixture: &mut Fixture, id: u64, width: u32) -> Result<FrameStamp, Box<dyn Error>> {
    fixture.geometry(id, width, 40)?;
    fresh_frame(fixture)
}

/// Wait for the next frame to await acknowledgement. Real worker progress is awaited on its wake;
/// injected cadence advances only while no admitted work remains.
fn fresh_frame(fixture: &mut Fixture) -> Result<FrameStamp, Box<dyn Error>> {
    for _ in 0..500 {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        if let Some(frame) = prepared_frame(&fixture.canvas).filter(|frame| frame.content.pending())
        {
            return Ok(frame.content.target());
        }
        fixture.wait_preview_progress(observed)?;
    }
    Err("No fresh frame was offered".into())
}

fn session(fixture: &mut Fixture) -> Result<&mut Session, Box<dyn Error>> {
    Ok(fixture.canvas.session.as_mut().ok_or("Session missing")?)
}

/// The accepted frame the composition keeps showing, if the canvas retains one.
fn retained(fixture: &Fixture) -> Option<FrameStamp> {
    match fixture.canvas.presentation() {
        MainCanvasPresentation::Retain(retained) => Some(retained.target),
        MainCanvasPresentation::Frame(_) | MainCanvasPresentation::Hide => None,
    }
}

/// The accepted frame on screen: retained pixels, or a frame with nothing awaiting acknowledgement.
fn shown(fixture: &Fixture) -> Option<FrameStamp> {
    match fixture.canvas.presentation() {
        MainCanvasPresentation::Retain(retained) => Some(retained.target),
        MainCanvasPresentation::Frame(frame) if !frame.content.pending() => {
            Some(frame.content.target())
        }
        MainCanvasPresentation::Frame(_) | MainCanvasPresentation::Hide => None,
    }
}

fn hidden_geometry() -> Value {
    json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":80,"height":40}, "clip":{"x":10,"y":20,"width":0,"height":0},
        "physicalPixelsPerCssPixel":1, "visible":false, "focused":false
    }})
}

/// Require that `rejected` was dropped silently: the pending request `id` stays open, the last
/// accepted frame stays the one shown, and a different fresh frame follows. That fresh frame is
/// accepted here and must complete the request successfully; it is returned.
fn assert_dropped(
    fixture: &mut Fixture,
    id: u64,
    accepted: FrameStamp,
    rejected: FrameStamp,
) -> Result<FrameStamp, Box<dyn Error>> {
    assert!(fixture.canvas.take_command().is_none(), "A dropped frame answered the request");
    assert_eq!(retained(fixture), Some(accepted), "The last accepted frame left the screen");
    let fresh = fresh_frame(fixture)?;
    assert_ne!(fresh, rejected, "The rejected frame was offered again");
    fixture.canvas.presented(fresh)?;
    fixture.take(id, "applied")?;
    assert_eq!(shown(fixture), Some(fresh));
    Ok(fresh)
}

#[test]
fn a_late_acknowledgement_of_a_replaced_frame_is_ignored() -> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    let second = next_frame(&mut fixture, 5, 90)?;
    assert_ne!(first, second);
    fixture.canvas.presented(first)?;
    assert!(fixture.canvas.take_command().is_none(), "A stale frame resolved the request");
    assert_eq!(fresh_frame(&mut fixture)?, second);
    fixture.canvas.presented(second)?;
    fixture.take(5, "applied")?;
    fixture.finish()
}

#[test]
fn a_frame_acknowledged_while_a_hide_is_pending_is_ignored() -> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    fixture.send(5, "surface.geometry", hidden_geometry())?;
    fixture.canvas.presented(first)?;
    assert!(fixture.canvas.take_command().is_none(), "The hide resolved without its native hide");
    let identity = fixture.identity;
    fixture.canvas.hidden(identity)?;
    fixture.take(5, "applied")?;
    fixture.finish()
}

#[test]
fn a_foreign_cpu_frame_is_ignored_and_changes_nothing() -> Result<(), Box<dyn Error>> {
    use sugarsubstitute_desktop::canvas::Lifetime;
    let (mut fixture, first) = committed()?;
    let second = next_frame(&mut fixture, 5, 90)?;
    let foreign = FrameStamp { surface: second.surface, generation: second.generation.next()? };
    let cause = session(&mut fixture)?
        .did_present_cpu(foreign)
        .err()
        .ok_or("A foreign frame was accepted")?;
    assert!(
        matches!(cause.downcast_ref::<Unaccepted>(), Some(Unaccepted::Ignored(Ignored::Foreign))),
        "{cause}"
    );
    assert_eq!(fresh_frame(&mut fixture)?, second);
    assert_eq!(retained(&fixture), None);
    fixture.canvas.presented(second)?;
    fixture.take(5, "applied")?;
    assert_ne!(first, second);
    fixture.finish()
}

#[test]
fn a_frame_without_evidence_is_dropped_and_the_last_accepted_frame_stays_shown()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    let second = next_frame(&mut fixture, 5, 90)?;
    session(&mut fixture)?.lose_pending_cpu_evidence();
    fixture.canvas.presented(second)?;
    assert_dropped(&mut fixture, 5, first, second)?;
    fixture.geometry(6, 100, 40)?;
    let next = fresh_frame(&mut fixture)?;
    fixture.canvas.presented(next)?;
    fixture.take(6, "applied")?;
    fixture.finish()
}

#[test]
fn a_frame_whose_evidence_disagrees_with_its_projection_is_dropped() -> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    let second = next_frame(&mut fixture, 5, 90)?;
    session(&mut fixture)?.skew_pending_cpu_evidence()?;
    fixture.canvas.presented(second)?;
    assert_dropped(&mut fixture, 5, first, second)?;
    fixture.finish()
}

#[test]
fn a_frame_drawn_for_an_earlier_backdrop_is_dropped() -> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    let second = next_frame(&mut fixture, 5, 90)?;
    session(&mut fixture)?.advance_backdrop_generation();
    fixture.canvas.presented(second)?;
    assert!(fixture.canvas.take_command().is_none(), "A dropped frame answered the request");
    assert_eq!(retained(&fixture), Some(first));
    // The forced generation never reaches the attachment's own, so the request stays open for
    // the geometry that really changes the backdrop; the fresh frame is still offered and accepted.
    let fresh = fresh_frame(&mut fixture)?;
    assert_ne!(fresh, second, "The rejected frame was offered again");
    fixture.canvas.presented(fresh)?;
    assert_eq!(shown(&fixture), Some(fresh));
    fixture.finish()
}

#[test]
fn an_acknowledgement_while_no_frame_is_selected_leaves_the_selection_pending()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    fixture.select(5, "tall.png")?;
    // The accepted frame retained while the selection loads is acknowledged again: a duplicate.
    fixture.canvas.presented(first)?;
    assert!(
        fixture.canvas.take_command().is_none(),
        "A duplicate acknowledgement resolved the selection"
    );
    // A frame the canvas never offered, while no frame is selected, cannot be accepted.
    let unknown = FrameStamp { surface: first.surface, generation: first.generation.next()? };
    fixture.canvas.presented(unknown)?;
    assert!(fixture.canvas.take_command().is_none(), "A dropped frame answered the selection");
    assert_eq!(retained(&fixture), Some(first));
    fixture.complete(5)?;
    fixture.finish()
}

#[test]
fn an_acknowledgement_after_the_attachment_was_hidden_is_dropped() -> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    fixture.send(5, "surface.geometry", hidden_geometry())?;
    let identity = fixture.identity;
    fixture.canvas.hidden(identity)?;
    fixture.take(5, "applied")?;
    fixture.canvas.presented(first)?;
    assert!(fixture.canvas.take_command().is_none());
    fixture.finish()
}

/// Reject `frame` for lost evidence, require `accepted` to stay shown and every pending request
/// to stay open, and return the fresh frame the canvas then offers.
fn reject(
    fixture: &mut Fixture,
    frame: FrameStamp,
    accepted: FrameStamp,
) -> Result<FrameStamp, Box<dyn Error>> {
    session(fixture)?.lose_pending_cpu_evidence();
    fixture.canvas.presented(frame)?;
    assert!(fixture.canvas.take_command().is_none(), "A dropped frame answered a request");
    assert_eq!(retained(fixture), Some(accepted), "The last accepted frame left the screen");
    fresh_frame(fixture)
}

#[test]
fn three_rejected_frames_in_a_row_present_the_attachment_again_and_it_recovers()
-> Result<(), Box<dyn Error>> {
    let lines = DiagnosticLines::default();
    let _capture = lines.observe();
    let (mut fixture, first) = committed()?;
    let mut frame = next_frame(&mut fixture, 5, 90)?;
    for round in 1..=3 {
        assert!(!lines.contains_operation("canvas_attachment_represented")?, "round {round}");
        frame = reject(&mut fixture, frame, first)?;
    }
    assert!(lines.contains_operation("canvas_attachment_represented")?);
    // The geometry request waited through every rejection and the re-presentation.
    fixture.canvas.presented(frame)?;
    fixture.take(5, "applied")?;
    assert_eq!(shown(&fixture), Some(frame));
    fixture.geometry(6, 100, 40)?;
    let next = fresh_frame(&mut fixture)?;
    fixture.canvas.presented(next)?;
    fixture.take(6, "applied")?;
    fixture.finish()
}

#[test]
fn an_accepted_frame_resets_the_count_of_rejected_frames() -> Result<(), Box<dyn Error>> {
    let lines = DiagnosticLines::default();
    let _capture = lines.observe();
    let (mut fixture, first) = committed()?;
    let mut frame = next_frame(&mut fixture, 5, 90)?;
    frame = reject(&mut fixture, frame, first)?;
    frame = reject(&mut fixture, frame, first)?;
    fixture.canvas.presented(frame)?;
    fixture.take(5, "applied")?;
    assert_eq!(shown(&fixture), Some(frame));
    let accepted = frame;
    fixture.geometry(6, 100, 40)?;
    frame = fresh_frame(&mut fixture)?;
    frame = reject(&mut fixture, frame, accepted)?;
    frame = reject(&mut fixture, frame, accepted)?;
    assert!(!lines.contains_operation("canvas_attachment_represented")?);
    frame = reject(&mut fixture, frame, accepted)?;
    assert!(lines.contains_operation("canvas_attachment_represented")?);
    fixture.canvas.presented(frame)?;
    fixture.take(6, "applied")?;
    fixture.finish()
}

#[test]
fn a_commit_waits_for_the_next_accepted_frame_after_a_frame_is_dropped()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.send(4, "surface.commit", json!({}))?;
    let frame = fixture.pending()?;
    session(&mut fixture)?.lose_pending_cpu_evidence();
    fixture.canvas.presented(frame)?;
    assert!(fixture.canvas.take_command().is_none(), "A dropped frame answered the commit");
    fixture.complete(4)?;
    fixture.finish()
}

#[test]
fn a_selection_waits_for_the_next_accepted_frame_after_a_frame_is_dropped()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    fixture.select(5, "tall.png")?;
    let frame = fresh_frame(&mut fixture)?;
    assert_ne!(frame, first);
    session(&mut fixture)?.lose_pending_cpu_evidence();
    fixture.canvas.presented(frame)?;
    assert!(fixture.canvas.take_command().is_none(), "A dropped frame answered the selection");
    assert_eq!(retained(&fixture), Some(first));
    fixture.complete(5)?;
    fixture.finish()
}

#[test]
fn a_viewport_command_waits_for_the_next_accepted_frame_after_a_frame_is_dropped()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, first) = committed()?;
    fixture.viewport(5, json!({"type":"native"}))?;
    fixture.advance(40);
    let frame = fresh_frame(&mut fixture)?;
    session(&mut fixture)?.lose_pending_cpu_evidence();
    fixture.canvas.presented(frame)?;
    assert!(fixture.canvas.take_command().is_none(), "A dropped frame answered the command");
    assert_eq!(retained(&fixture), Some(first));
    fixture.complete(5)?;
    fixture.finish()
}

#[test]
fn lost_frame_accounting_stays_fatal() -> Result<(), Box<dyn Error>> {
    let (mut fixture, _) = committed()?;
    let second = next_frame(&mut fixture, 5, 90)?;
    session(&mut fixture)?.forget_frame_in_pacing(second)?;
    assert!(fixture.canvas.presented(second).is_err());
    Ok(())
}
