// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore image-relative cameras on selection and on the shown image, reporting their centers.

use super::*;
use viewport::fixture::Fixture;

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
