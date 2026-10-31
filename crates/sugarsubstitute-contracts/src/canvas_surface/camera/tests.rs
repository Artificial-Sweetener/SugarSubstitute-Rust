// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match shared settled-camera vectors and encode typed host changes in their exact shape.

use super::*;
use crate::canvas_surface::CanvasViewportMode;
use serde_json::{Value, json};
use std::error::Error;

fn vectors() -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str::<Value>(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/sugarcubes-canvas/inspection_vectors.json"
    )))?["cameraChanges"]
        .clone())
}

#[test]
fn shared_camera_vectors_preserve_the_generated_bounds() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    for vector in vectors["valid"].as_array().ok_or("Missing valid cameras")? {
        let decoded: wire::CanvasSurfaceCameraChanged =
            serde_json::from_value(vector["value"].clone())?;
        assert!(decoded.valid(), "{}", vector["name"]);
    }
    for vector in vectors["invalid"].as_array().ok_or("Missing invalid cameras")? {
        let decoded =
            serde_json::from_value::<wire::CanvasSurfaceCameraChanged>(vector["value"].clone());
        assert!(!decoded.is_ok_and(|change| change.valid()), "{}", vector["name"]);
    }
    Ok(())
}

#[test]
fn settled_cameras_encode_as_asynchronous_notifications() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let mut expected = vectors["valid"][0]["value"].clone();
    // Rust encodes the shared integral scale as a float; JSON numbers compare by representation.
    expected["change"]["viewport"]["physicalScale"] = json!(2.0);
    let address =
        CanvasSurfaceAddress::decode(&serde_json::to_vec(&expected["change"]["address"])?)?;
    let viewport = CanvasViewportState::new(CanvasViewportMode::Custom, 2.0, 1000, 800)?
        .with_center(0.25, 0.75)?;
    let change = CanvasSurfaceCameraChange::new(address.clone(), 4, 7, viewport)?;
    assert_eq!((change.sequence(), change.selection_revision()), (4, 7));
    assert_eq!(change.viewport(), viewport);
    assert_eq!(change.address(), &address);
    let session = PanelSession::new("surface-inspection-session".into())?;
    let encoded: Value = serde_json::from_slice(&change.encode(&session)?)?;
    assert_eq!(encoded, expected);
    assert_eq!(encoded["requestId"], json!(0));
    for (sequence, revision) in [(0, 7), (4, 0), (9_007_199_254_740_992, 7)] {
        assert!(
            CanvasSurfaceCameraChange::new(address.clone(), sequence, revision, viewport).is_err()
        );
    }
    Ok(())
}
