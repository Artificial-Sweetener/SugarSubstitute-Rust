// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore image-relative cameras on selection and on the shown image, reporting their centers.

use super::*;
use sugarsubstitute_desktop::canvas::{MainCanvasButton, MainCanvasGesture, MainCanvasInput};
use viewport::fixture::{Execution, Fixture};

/// An inspection at `zoom` physical pixels per pixel of the 320x160 `wide.png` output.
fn inspection(center: [f64; 2], zoom: f64) -> Value {
    json!({
        "center":{"x":center[0], "y":center[1]},
        "span":{"x":1.0 / (320.0 * zoom), "y":1.0 / (160.0 * zoom)}
    })
}

fn close(actual: &Value, expected: f64) -> bool {
    actual.as_f64().is_some_and(|value| (value - expected).abs() <= 1e-9 * expected.max(1.0))
}

/// Assert a reported camera's mode, scale and normalized center.
fn assert_camera(viewport: &Value, mode: &str, zoom: f64, center: [f64; 2]) {
    assert_eq!(viewport["mode"], mode, "{viewport}");
    assert!(close(&viewport["physicalScale"], zoom), "{viewport}");
    assert!(close(&viewport["center"]["x"], center[0]), "{viewport}");
    assert!(close(&viewport["center"]["y"], center[1]), "{viewport}");
}

#[test]
fn a_selection_carrying_an_inspection_presents_its_first_frame_at_that_camera()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    fixture.send(
        3,
        "surface.select",
        json!({
            "itemId":"result:wide.png",
            "file":{"filename":"wide.png","subfolder":"","type":"output"},
            "inspection":inspection([0.25, 0.5], 2.0)
        }),
    )?;
    wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
    let first = fixture.commit(4)?;
    assert!((first.viewport.zoom() - 2.0).abs() < 1e-9);
    let center = first.viewport.document_at([40.0, 20.0])?;
    assert!((center[0] - 80.0).abs() < 1e-9 && (center[1] - 80.0).abs() < 1e-9);
    fixture.viewport(5, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(5)?["viewport"], "custom", 2.0, [0.25, 0.5]);
    fixture.select(6, "tall.png")?;
    fixture.complete(6)?;
    fixture.viewport(7, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(7)?["viewport"], "fit", 0.125, [0.5, 0.5]);
    fixture.finish()
}

#[test]
fn inspecting_the_shown_image_applies_and_reports_the_camera_immediately()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(5)?["viewport"], "fit", 0.25, [0.5, 0.5]);
    fixture.viewport(6, json!({"type":"inspect","inspection":inspection([0.75, 0.25], 1.5)}))?;
    assert_camera(&fixture.complete(6)?["viewport"], "custom", 1.5, [0.75, 0.25]);
    let accepted = fixture.accepted()?;
    assert!((accepted.viewport.zoom() - 1.5).abs() < 1e-9);
    fixture.finish()
}

#[test]
fn inspections_at_fit_and_one_to_one_take_their_semantic_modes() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"inspect","inspection":inspection([0.5, 0.5], 1.0)}))?;
    assert_camera(&fixture.complete(5)?["viewport"], "native", 1.0, [0.5, 0.5]);
    fixture.viewport(6, json!({"type":"inspect","inspection":inspection([0.1, 0.9], 0.25)}))?;
    assert_camera(&fixture.complete(6)?["viewport"], "fit", 0.25, [0.5, 0.5]);
    fixture.finish()
}

#[test]
fn inspection_requires_a_committed_final() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.viewport(4, json!({"type":"inspect","inspection":inspection([0.5, 0.5], 2.0)}))?;
    fixture.take(4, "unavailable")?;
    fixture.commit(5)?;
    fixture.viewport(6, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(6)?["viewport"], "fit", 0.25, [0.5, 0.5]);
    fixture.finish()
}

/// Route gestures through the same entry point as the main-loop observation route.
fn route(fixture: &mut Fixture, gestures: &[MainCanvasGesture]) -> Result<(), Box<dyn Error>> {
    for gesture in gestures {
        fixture.canvas.input(MainCanvasInput { identity: fixture.identity, gesture: *gesture })?;
    }
    Ok(())
}

fn button(pressed: bool) -> MainCanvasGesture {
    MainCanvasGesture::Button { button: MainCanvasButton::Primary, pressed }
}

/// Advance injected time, present every frame and return the settled-camera notifications.
fn settle(fixture: &mut Fixture) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut cameras = Vec::new();
    for _ in 0..40 {
        while let Some(command) = fixture.canvas.take_command() {
            let DesktopStartupCommand::SendSurfacePresentation(change) = command else {
                return Err("Unexpected ordinary reply while settling".into());
            };
            let value: Value = serde_json::from_slice(change.payload())?;
            if value["type"] == "surface.camera.changed" {
                cameras.push(value);
            }
        }
        let observed = fixture.wake.revision();
        fixture.advance(100);
        fixture.pump()?;
        if let Some(target) = prepared_frame(&fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| frame.content.target())
        {
            fixture.canvas.presented(target)?;
        } else if fixture.execution()? == Execution::Running {
            fixture.wake.wait_after(observed)?;
        }
    }
    Ok(cameras)
}

/// Commit the selected final and observe presentation like the page's preview owner.
fn watched() -> Result<Fixture, Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    preview::watch(&mut fixture, 5, true)?;
    assert!(settle(&mut fixture)?.is_empty());
    Ok(fixture)
}

#[test]
fn a_settled_wheel_gesture_publishes_its_camera_exactly_once() -> Result<(), Box<dyn Error>> {
    let mut fixture = watched()?;
    route(
        &mut fixture,
        &[MainCanvasGesture::Pointer([30.0, 30.0]), MainCanvasGesture::Wheel(1.0)],
    )?;
    let cameras = settle(&mut fixture)?;
    assert_eq!(cameras.len(), 1, "{cameras:?}");
    let change = &cameras[0]["change"];
    assert_eq!(cameras[0]["requestId"], 0);
    assert_eq!(change["selectionRevision"], 3);
    assert_eq!(change["viewport"]["mode"], "custom");
    let zoom = fixture.accepted()?.viewport.zoom();
    assert!(zoom > 0.25 && close(&change["viewport"]["physicalScale"], zoom), "{change}");
    fixture.viewport(6, json!({"type":"read"}))?;
    assert_eq!(fixture.complete(6)?["viewport"], change["viewport"]);
    assert!(settle(&mut fixture)?.is_empty());
    fixture.finish()
}

#[test]
fn a_drag_publishes_only_after_its_release() -> Result<(), Box<dyn Error>> {
    let mut fixture = watched()?;
    fixture.viewport(6, json!({"type":"zoom","factor":4}))?;
    fixture.complete(6)?;
    assert!(settle(&mut fixture)?.is_empty());
    route(
        &mut fixture,
        &[
            MainCanvasGesture::Pointer([50.0, 40.0]),
            button(true),
            MainCanvasGesture::Pointer([60.0, 44.0]),
        ],
    )?;
    assert!(settle(&mut fixture)?.is_empty());
    route(&mut fixture, &[button(false)])?;
    let cameras = settle(&mut fixture)?;
    assert_eq!(cameras.len(), 1, "{cameras:?}");
    let viewport = &cameras[0]["change"]["viewport"];
    assert_eq!(viewport["mode"], "custom");
    assert!(close(&viewport["physicalScale"], 1.0), "{viewport}");
    assert!(!close(&viewport["center"]["x"], 0.5), "{viewport}");
    route(&mut fixture, &[MainCanvasGesture::Pointer([20.0, 25.0])])?;
    assert!(settle(&mut fixture)?.is_empty());
    fixture.finish()
}

#[test]
fn cancelling_a_held_drag_publishes_its_final_camera() -> Result<(), Box<dyn Error>> {
    let mut fixture = watched()?;
    fixture.viewport(6, json!({"type":"zoom","factor":4}))?;
    fixture.complete(6)?;
    route(&mut fixture, &[MainCanvasGesture::Pointer([50.0, 40.0]), button(true)])?;
    route(&mut fixture, &[MainCanvasGesture::Pointer([40.0, 35.0])])?;
    assert!(settle(&mut fixture)?.is_empty());
    route(&mut fixture, &[MainCanvasGesture::Cancel])?;
    let cameras = settle(&mut fixture)?;
    assert_eq!(cameras.len(), 1, "{cameras:?}");
    let zoom = fixture.accepted()?.viewport.zoom();
    assert!(close(&cameras[0]["change"]["viewport"]["physicalScale"], zoom));
    fixture.finish()
}

#[test]
fn a_shown_preview_publishes_no_gesture_camera() -> Result<(), Box<dyn Error>> {
    let mut fixture = watched()?;
    fixture.viewport(6, json!({"type":"zoom","factor":4}))?;
    fixture.complete(6)?;
    route(&mut fixture, &[MainCanvasGesture::Pointer([50.0, 40.0]), button(true)])?;
    preview::transfer(&mut fixture, 200, "preview:1", preview::JPEG)?;
    let target = preview::wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    route(&mut fixture, &[button(false)])?;
    assert!(settle(&mut fixture)?.is_empty());
    preview::drain(&mut fixture)?;
    fixture.finish()
}

#[test]
fn a_routed_double_click_from_a_zoomed_camera_publishes_fit() -> Result<(), Box<dyn Error>> {
    let mut fixture = watched()?;
    fixture.viewport(6, json!({"type":"zoom","factor":4}))?;
    fixture.complete(6)?;
    assert!(settle(&mut fixture)?.is_empty());
    let point = [50.0, 40.0];
    // The exact order the host routing emits for two primary clicks within the Windows rule.
    route(
        &mut fixture,
        &[
            MainCanvasGesture::Pointer(point),
            button(true),
            MainCanvasGesture::Pointer(point),
            button(false),
            MainCanvasGesture::Pointer(point),
            button(true),
            MainCanvasGesture::DoubleClick(point),
            MainCanvasGesture::Pointer(point),
            button(false),
        ],
    )?;
    let cameras = settle(&mut fixture)?;
    let published = cameras.last().ok_or("The double-click published no camera")?;
    let viewport = &published["change"]["viewport"];
    assert_eq!(viewport["mode"], "fit", "{cameras:?}");
    assert!(close(&viewport["physicalScale"], 0.25), "{viewport}");
    fixture.viewport(7, json!({"type":"read"}))?;
    assert_eq!(fixture.complete(7)?["viewport"], *viewport);
    fixture.finish()
}
