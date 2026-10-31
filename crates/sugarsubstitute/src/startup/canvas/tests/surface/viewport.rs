// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive addressed shared viewport operations through real CPU pixels and native acceptance.

use super::*;
use crate::native_canvas::session::{AcceptedCamera, CameraCommandResult};
use sugarsubstitute_canvas::FrameStamp;

pub(super) mod fixture;
use fixture::Fixture;

fn snapshot(value: &Value, id: u64, mode: &str, scale: f64, dimensions: [u32; 2]) {
    assert_eq!(value["type"], "surface.viewport.reply");
    assert_eq!(value["requestId"], id);
    assert_eq!(value["revision"], id);
    assert_eq!(value["address"]["attachmentId"], "attachment:one");
    assert_eq!(value["viewport"]["mode"], mode);
    assert_eq!(value["viewport"]["physicalScale"], scale);
    assert_eq!(
        value["viewport"]["dimensions"],
        json!({"width":dimensions[0],"height":dimensions[1]})
    );
}

#[test]
fn addressed_commands_report_only_the_exact_accepted_camera_and_reuse_unchanged_pixels()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.viewport(4, json!({"type":"read"}))?;
    fixture.take(4, "unavailable")?;
    let fitted = fixture.commit(5)?;
    fixture.viewport(6, json!({"type":"read"}))?;
    snapshot(&fixture.take(6, "applied")?, 6, "fit", 0.25, [320, 160]);
    fixture.viewport(7, json!({"type":"fit"}))?;
    snapshot(&fixture.take(7, "applied")?, 7, "fit", 0.25, [320, 160]);
    assert_eq!(fixture.accepted()?, fitted);
    assert!(!prepared_frame(&fixture.canvas).ok_or("Accepted frame absent")?.frame.pending);
    fixture.send(
        8,
        "surface.viewport.request",
        json!({"revision":7,"command":{"type":"native"}}),
    )?;
    fixture.take(8, "stale")?;
    fixture.send(
        9,
        "surface.viewport.request",
        json!({
            "address":{"sessionId":"canvas:workflow","viewId":"view:one","windowId":"window:one",
                "attachmentId":"attachment:foreign","generation":1},
            "command":{"type":"native"}
        }),
    )?;
    fixture.take(9, "stale")?;
    assert_eq!(fixture.accepted()?, fitted);

    fixture.viewport(10, json!({"type":"native"}))?;
    fixture.canvas.presented(fitted.frame)?;
    assert!(fixture.canvas.take_command().is_none());
    fixture.advance(40);
    let intermediate = fixture.pending()?;
    fixture.canvas.presented(intermediate)?;
    let camera = fixture.accepted()?;
    assert!(camera.viewport.zoom() > 0.25 && camera.viewport.zoom() < 1.0);
    assert!(fixture.canvas.take_command().is_none());
    snapshot(&fixture.complete(10)?, 10, "native", 1.0, [320, 160]);
    let native = fixture.accepted()?;
    assert_eq!(native.viewport.origin(), [120.0, 60.0]);
    assert_eq!(native.document, fitted.document);
    assert_eq!(native.source_revision, fitted.source_revision);
    assert_ne!(native.frame, intermediate);

    fixture.viewport(11, json!({"type":"zoom","factor":2}))?;
    snapshot(&fixture.complete(11)?, 11, "custom", 2.0, [320, 160]);
    fixture.viewport(12, json!({"type":"pan","units":"viewport","x":0.25,"y":-0.5}))?;
    snapshot(&fixture.complete(12)?, 12, "custom", 2.0, [320, 160]);
    assert_eq!(fixture.accepted()?.viewport.origin(), [130.0, 80.0]);
    fixture.viewport(13, json!({"type":"fit"}))?;
    snapshot(&fixture.complete(13)?, 13, "fit", 0.25, [320, 160]);
    let unchanged = fixture.accepted()?;
    fixture.viewport(14, json!({"type":"fit"}))?;
    snapshot(&fixture.take(14, "applied")?, 14, "fit", 0.25, [320, 160]);
    fixture.viewport(15, json!({"type":"read"}))?;
    snapshot(&fixture.take(15, "applied")?, 15, "fit", 0.25, [320, 160]);
    assert_eq!(fixture.accepted()?, unchanged);
    fixture.finish()
}

#[test]
fn camera_cancel_keeps_ordinary_and_cancel_credit_distinct_and_cannot_stop_a_newer_request()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let original = fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"native"}))?;
    fixture.advance(40);
    fixture.cancel(5)?;
    assert!(fixture.memory.claim(1).is_err());
    let Some(DesktopStartupCommand::SendSurface(ordinary)) = fixture.canvas.take_command() else {
        return Err("Cancelled camera lost its ordinary terminal".into());
    };
    let value: Value = serde_json::from_slice(ordinary.payload())?;
    assert_eq!(ordinary.identity(), fixture.identity);
    assert_eq!(value["requestId"], 5);
    assert_eq!(value["revision"], 5);
    assert_eq!(value["session"], fixture.panel.as_str());
    assert_eq!(value["status"], "cancelled");
    let Some(DesktopStartupCommand::CompletePanelIngress(receipt)) = fixture.canvas.take_command()
    else {
        return Err("Cancelled camera lost its independent consumption receipt".into());
    };
    assert_eq!(receipt.identity(), fixture.identity);
    assert!(fixture.memory.claim(1).is_err());
    drop(ordinary);
    drop(fixture.memory.claim(64 * 1024)?);
    assert!(fixture.memory.claim(64 * 1024 + 1).is_err());
    drop(receipt);
    assert_eq!(fixture.accepted()?, original);
    let CameraCommandResult::Accepted(stopped) =
        fixture.canvas.session.as_ref().ok_or("Session absent")?.camera_intent()
    else {
        return Err("Cancelled camera no longer has current content".into());
    };
    assert_eq!(stopped.mode(), sugarsubstitute_canvas::ZoomMode::Custom);

    fixture.viewport(6, json!({"type":"fit"}))?;
    let current = fixture.canvas.session.as_ref().ok_or("Session absent")?.camera_intent();
    fixture.cancel(5)?;
    cancel_consumed(&mut fixture.canvas, fixture.identity)?;
    assert!(fixture.canvas.take_command().is_none());
    assert_eq!(fixture.canvas.session.as_ref().ok_or("Session absent")?.camera_intent(), current);
    snapshot(&fixture.complete(6)?, 6, "fit", 0.25, [320, 160]);
    fixture.viewport(7, json!({"type":"read"}))?;
    snapshot(&fixture.take(7, "applied")?, 7, "fit", 0.25, [320, 160]);
    fixture.viewport(8, json!({"type":"native"}))?;
    fixture.advance(40);
    fixture.cancel(8)?;
    fixture.take(8, "cancelled")?;
    cancel_consumed(&mut fixture.canvas, fixture.identity)?;
    fixture.geometry(9, 100, 50)?;
    fixture.complete(9)?;
    fixture.viewport(10, json!({"type":"read"}))?;
    snapshot(&fixture.complete(10)?, 10, "custom", 0.8125, [320, 160]);
    fixture.viewport(11, json!({"type":"native"}))?;
    fixture.advance(40);
    fixture.cancel(11)?;
    fixture.take(11, "cancelled")?;
    cancel_consumed(&mut fixture.canvas, fixture.identity)?;
    fixture.select(12, "tall.png")?;
    fixture.complete(12)?;
    fixture.viewport(13, json!({"type":"read"}))?;
    snapshot(&fixture.complete(13)?, 13, "fit", 0.15625, [160, 320]);
    fixture.finish()
}

#[test]
fn resize_and_source_retirement_fence_shared_camera_waiters_and_their_old_frames()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"native"}))?;
    fixture.advance(40);
    let old_frame = fixture.pending()?;
    fixture
        .canvas
        .session
        .as_mut()
        .ok_or("Session absent")?
        .resize(sugarsubstitute_image_ops::Extent::new(100, 50)?)?;
    fixture.pump()?;
    fixture.take(5, "stale")?;
    assert!(fixture.canvas.presented(old_frame).is_err());
    fixture.geometry(6, 100, 50)?;
    fixture.complete(6)?;
    fixture.viewport(7, json!({"type":"read"}))?;
    snapshot(&fixture.complete(7)?, 7, "native", 1.0, [320, 160]);
    let retained = fixture.accepted()?;
    fixture.viewport(8, json!({"type":"zoom","factor":2}))?;
    fixture.canvas.session.as_mut().ok_or("Session absent")?.cancel_selection()?;
    fixture.pump()?;
    fixture.take(8, "stale")?;
    assert!(fixture.canvas.presented(retained.frame).is_err());
    let MainCanvasPresentation::Retain(frame) = fixture.canvas.presentation() else {
        return Err("Source retirement released acknowledged native pixels".into());
    };
    assert_eq!(frame.target, retained.frame);
    fixture.viewport(9, json!({"type":"read"}))?;
    fixture.take(9, "unavailable")?;
    fixture.select(10, "tall.png")?;
    fixture.complete(10)?;
    fixture.viewport(11, json!({"type":"read"}))?;
    snapshot(&fixture.complete(11)?, 11, "fit", 0.15625, [160, 320]);
    assert_ne!(fixture.accepted()?.source_revision, retained.source_revision);
    fixture.finish()
}

#[test]
fn viewport_native_scale_and_fractional_pan_use_the_projected_physical_extent_once()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::with_density(1.5)?;
    let fitted = fixture.commit(4)?;
    assert_eq!(fitted.viewport.surface(), sugarsubstitute_image_ops::Extent::new(120, 60)?);
    let frame = prepared_frame(&fixture.canvas).ok_or("Physical frame absent")?;
    assert_eq!(frame.placement.viewport, PhysicalRect { x: 15, y: 30, width: 120, height: 60 });
    fixture.viewport(5, json!({"type":"read"}))?;
    snapshot(&fixture.take(5, "applied")?, 5, "fit", 0.375, [320, 160]);
    fixture.viewport(6, json!({"type":"native"}))?;
    snapshot(&fixture.complete(6)?, 6, "native", 1.0, [320, 160]);
    assert_eq!(fixture.accepted()?.viewport.origin(), [100.0, 50.0]);
    fixture.viewport(7, json!({"type":"pan","units":"viewport","x":0.25,"y":0}))?;
    snapshot(&fixture.complete(7)?, 7, "native", 1.0, [320, 160]);
    assert_eq!(fixture.accepted()?.viewport.origin(), [70.0, 50.0]);
    fixture.finish()
}

#[test]
fn current_native_failure_terminates_the_shared_camera_request_without_losing_the_cause()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"native"}))?;
    fixture
        .canvas
        .session
        .as_mut()
        .ok_or("Session absent")?
        .selected(Err(Box::new(std::io::Error::from_raw_os_error(87))))?;
    fixture.pump()?;
    let terminal = fixture.take(5, "error")?;
    assert_eq!(terminal["type"], "surface.reply");
    assert_eq!(terminal["revision"], 5);
    assert!(terminal.get("viewport").is_none());
    assert!(fixture.canvas.take_command().is_none());
    fixture.finish_failed(87)
}
