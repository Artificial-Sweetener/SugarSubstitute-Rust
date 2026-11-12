// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify exact schema-two inspection at real source handoff and geometry acceptance.

use super::*;

#[test]
fn rejected_discovery_fences_mutations_until_matching_feature_schema_is_negotiated()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let original = fixture.commit(4)?;
    for (id, schema, expected) in [(5, 1, "unsupported"), (7, 2, "available")] {
        let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":fixture.panel.as_str(),
            "requestId":id, "type":"surface.capabilities.request", "schema":schema,
        }))?)?;
        fixture.canvas.surface(
            fixture.identity,
            message,
            None,
            fixture.memory.claim(64 * 1024)?,
        )?;
        let reply = fixture.take(id, expected)?;
        assert_eq!(reply["capabilities"]["schema"], schema);
        if schema == 1 {
            assert_eq!(reply["capabilities"]["viewportCommands"], json!([]));
            fixture.viewport(
                6,
                json!({"type":"inspect", "inspection":inspection([-3.25,2.75],2.0)}),
            )?;
            fixture.take(6, "unavailable")?;
            assert_eq!(fixture.accepted()?.viewport, original.viewport);
        }
    }
    fixture.viewport(8, json!({"type":"inspect", "inspection":inspection([-3.25,2.75],2.0)}))?;
    assert_camera(&fixture.complete(8)?["viewport"], "custom", 2.0, [-3.25, 2.75]);
    fixture.finish()
}

#[test]
fn off_image_selection_presents_and_reports_its_exact_camera_before_and_after_resize()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::empty()?;
    let center = [-3.25, 2.75];
    fixture.send(
        3,
        "surface.select",
        json!({
            "itemId":"result:wide.png",
            "file":{"filename":"wide.png","subfolder":"","type":"output"},
            "inspection":inspection(center, 2.0)
        }),
    )?;
    fixture.wait_reply(3, "applied")?;
    let first = fixture.commit(4)?;
    assert_eq!(first.viewport.zoom(), 2.0);
    let point = first.viewport.document_at([40.0, 20.0])?;
    assert!((point[0] / 320.0 - center[0]).abs() < 1e-9);
    assert!((point[1] / 160.0 - center[1]).abs() < 1e-9);
    fixture.viewport(5, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(5)?["viewport"], "custom", 2.0, center);
    fixture.geometry(6, 100, 60)?;
    fixture.complete(6)?;
    fixture.viewport(7, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(7)?["viewport"], "custom", 2.0, center);
    fixture.send(
        8,
        "surface.select",
        json!({
            "itemId":"result:green.png",
            "file":{"filename":"green.png","subfolder":"","type":"output"},
            "inspection":inspection(center, 2.0)
        }),
    )?;
    fixture.complete(8)?;
    fixture.viewport(9, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(9)?["viewport"], "custom", 2.0, center);
    fixture.finish()
}

#[test]
fn nonrepresentable_inspection_returns_error_and_preserves_the_accepted_camera()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let original = fixture.commit(4)?;
    for (id, center, scale) in [(5, [f64::MAX, 0.5], 2.0), (6, [0.5, 0.5], 100_000.0)] {
        fixture.viewport(id, json!({"type":"inspect", "inspection":inspection(center, scale)}))?;
        let failure = fixture.take(id, "error")?;
        assert!(failure["message"].as_str().is_some_and(|text| text.contains("retained")));
        assert_eq!(fixture.accepted()?.viewport, original.viewport);
    }
    fixture.viewport(7, json!({"type":"read"}))?;
    assert_camera(&fixture.complete(7)?["viewport"], "fit", 0.25, [0.5, 0.5]);
    fixture.finish()
}

#[test]
fn inspect_at_the_fit_scale_keeps_an_off_center_camera_custom() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    let center = [-0.25, 1.5];
    fixture.viewport(5, json!({"type":"inspect", "inspection":inspection(center, 0.25)}))?;
    assert_camera(&fixture.complete(5)?["viewport"], "custom", 0.25, center);
    fixture.finish()
}
