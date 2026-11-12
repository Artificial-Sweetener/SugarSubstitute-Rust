// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match shared viewport fixtures and retain closed request and response boundaries.

use super::*;
use crate::canvas_surface::{
    CanvasSurfaceMessage, CanvasSurfaceOperation, CanvasSurfaceReplyStatus, CanvasSurfaceRequest,
};
use serde_json::{Value, json};
use std::error::Error;

fn vectors() -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_viewport_vectors.json"
    )))?)
}

fn request(value: &Value) -> Result<Box<CanvasSurfaceRequest>, Box<dyn Error>> {
    match CanvasSurfaceMessage::decode(&serde_json::to_vec(value)?)? {
        CanvasSurfaceMessage::Request(request) => Ok(request),
        _ => Err("Expected an attachment request".into()),
    }
}

#[test]
fn shared_viewport_request_vectors_match_the_web_boundary() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    for value in vectors["validRequests"].as_array().ok_or("Missing valid requests")? {
        let decoded = request(value)?;
        let expected = match value["command"]["type"].as_str().ok_or("Missing command type")? {
            "read" => CanvasViewportCommand::Read,
            "fit" => CanvasViewportCommand::Fit,
            "native" => CanvasViewportCommand::Native,
            "zoom" => CanvasViewportCommand::Zoom {
                factor: value["command"]["factor"].as_f64().ok_or("Missing factor")?,
            },
            "pan" => CanvasViewportCommand::Pan {
                x: value["command"]["x"].as_f64().ok_or("Missing x")?,
                y: value["command"]["y"].as_f64().ok_or("Missing y")?,
            },
            "wipe" => CanvasViewportCommand::Wipe {
                orientation: match value["command"]["orientation"].as_str() {
                    Some("vertical") => CanvasWipeOrientation::Vertical,
                    Some("horizontal") => CanvasWipeOrientation::Horizontal,
                    _ => return Err("Unknown fixture wipe orientation".into()),
                },
                split: value["command"]["split"].as_f64().ok_or("Missing split")?,
            },
            "inspect" => return Err("Inspection belongs to its own shared vectors".into()),
            _ => return Err("Unknown fixture command".into()),
        };
        assert_eq!(decoded.operation(), &CanvasSurfaceOperation::Viewport(expected));
        assert_eq!(decoded.session().as_str(), value["session"].as_str().ok_or("Missing session")?);
        assert_eq!(decoded.request_id(), value["requestId"].as_u64().ok_or("Missing request")?);
        assert_eq!(decoded.revision(), value["revision"].as_u64().ok_or("Missing revision")?);
    }
    for value in vectors["invalidRequests"].as_array().ok_or("Missing invalid requests")? {
        assert!(CanvasSurfaceMessage::decode(&serde_json::to_vec(value)?).is_err(), "{value}");
    }
    for value in vectors["invalidRequestJson"].as_array().ok_or("Missing invalid raw requests")? {
        assert!(
            CanvasSurfaceMessage::decode(value.as_str().ok_or("Missing raw JSON")?.as_bytes())
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn shared_viewport_reply_vectors_preserve_the_generated_bounds() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    for value in vectors["validReplies"].as_array().ok_or("Missing valid replies")? {
        let decoded: wire::CanvasSurfaceViewportReply = serde_json::from_value(value.clone())?;
        assert!(decoded.valid(), "{value}");
    }
    for value in vectors["invalidReplies"].as_array().ok_or("Missing invalid replies")? {
        let decoded = serde_json::from_value::<wire::CanvasSurfaceViewportReply>(value.clone());
        assert!(!decoded.is_ok_and(|reply| reply.valid()), "{value}");
    }
    Ok(())
}

#[test]
fn viewport_success_retains_correlation_and_requires_an_observed_snapshot()
-> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let value = &vectors["validRequests"][0];
    let request = request(value)?;
    let state = CanvasViewportState::new(CanvasViewportMode::Custom, 1.25, 640, 480)?;
    assert_eq!(state.mode(), CanvasViewportMode::Custom);
    assert_eq!(state.physical_scale(), 1.25);
    assert_eq!(state.dimensions(), (640, 480));
    let reply: Value = serde_json::from_slice(&request.viewport_reply(&state, &"🌈".repeat(512))?)?;
    assert_eq!(reply["type"], "surface.viewport.reply");
    assert_eq!(reply["status"], "applied");
    for key in ["session", "requestId", "address", "revision"] {
        assert_eq!(reply[key], value[key], "{key}");
    }
    assert_eq!(
        reply["viewport"],
        json!({
            "mode":"custom", "physicalScale":1.25, "dimensions":{"width":640,"height":480}
        })
    );
    assert!(request.viewport_reply(&state, &"🌈".repeat(513)).is_err());
    for status in [CanvasSurfaceReplyStatus::Ready, CanvasSurfaceReplyStatus::Applied] {
        assert!(request.reply(status, "").is_err());
    }
    for status in [
        CanvasSurfaceReplyStatus::Stale,
        CanvasSurfaceReplyStatus::Cancelled,
        CanvasSurfaceReplyStatus::Unavailable,
        CanvasSurfaceReplyStatus::Error,
    ] {
        let reply: Value = serde_json::from_slice(&request.reply(status, "")?)?;
        assert_eq!(reply["type"], "surface.reply");
        assert!(reply.get("viewport").is_none());
    }
    Ok(())
}

#[test]
fn viewport_snapshots_reject_nonfinite_scales_and_invalid_dimensions() -> Result<(), Box<dyn Error>>
{
    for mode in [CanvasViewportMode::Fit, CanvasViewportMode::Native, CanvasViewportMode::Custom] {
        for scale in [1e-9, 16384.0] {
            assert!(CanvasViewportState::new(mode, scale, 1, 2147483647).is_ok());
        }
    }
    for scale in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, 0.999e-9, 16384.1] {
        assert!(CanvasViewportState::new(CanvasViewportMode::Fit, scale, 1, 1).is_err());
    }
    for (width, height) in [(0, 1), (1, 0), (2147483648, 1), (1, u32::MAX)] {
        assert!(CanvasViewportState::new(CanvasViewportMode::Native, 1.0, width, height).is_err());
    }
    let vectors = vectors()?;
    let state = CanvasViewportState::new(CanvasViewportMode::Native, 1.0, 1, 1)?;
    let mut value = vectors["validRequests"][0].clone();
    value
        .as_object_mut()
        .ok_or("Missing request object")?
        .remove("command")
        .ok_or("Missing viewport command")?;
    for operation in ["surface.prepare", "surface.commit", "surface.release"] {
        value["type"] = json!(operation);
        let request = request(&value)?;
        assert!(request.viewport_reply(&state, "").is_err());
        let status = if operation == "surface.prepare" {
            CanvasSurfaceReplyStatus::Ready
        } else {
            CanvasSurfaceReplyStatus::Applied
        };
        assert!(request.reply(status, "").is_ok());
    }
    Ok(())
}

fn inspection_vectors() -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_inspection_vectors.json"
    )))?)
}

/// Read a vector inspection as its normalized `[center, span]` axis pairs.
fn pairs(value: &Value) -> Result<([f64; 2], [f64; 2]), Box<dyn Error>> {
    let axis = |name: &str, key: &str| value[name][key].as_f64().ok_or("Missing inspection axis");
    Ok(([axis("center", "x")?, axis("center", "y")?], [axis("span", "x")?, axis("span", "y")?]))
}

#[test]
fn shared_inspection_vectors_decode_selection_and_inspect_cameras() -> Result<(), Box<dyn Error>> {
    let vectors = inspection_vectors()?;
    for vector in vectors["validRequests"].as_array().ok_or("Missing valid requests")? {
        let value = &vector["value"];
        let decoded = request(value)?;
        let (inspection, expected) = match decoded.operation() {
            CanvasSurfaceOperation::Select { inspection: Some(inspection), comparison, .. } => {
                assert_eq!(comparison.is_some(), value.get("comparison").is_some());
                (inspection, pairs(&value["inspection"])?)
            }
            CanvasSurfaceOperation::Viewport(CanvasViewportCommand::Inspect(inspection)) => {
                (inspection, pairs(&value["command"]["inspection"])?)
            }
            operation => {
                return Err(format!("Unexpected inspection operation {operation:?}").into());
            }
        };
        assert_eq!((inspection.center(), inspection.span()), expected, "{}", vector["name"]);
    }
    for vector in vectors["invalidRequests"].as_array().ok_or("Missing invalid requests")? {
        let bytes = serde_json::to_vec(&vector["value"])?;
        assert!(CanvasSurfaceMessage::decode(&bytes).is_err(), "{}", vector["name"]);
    }
    for vector in vectors["validReplies"].as_array().ok_or("Missing valid replies")? {
        let decoded: wire::CanvasSurfaceViewportReply =
            serde_json::from_value(vector["value"].clone())?;
        assert!(decoded.valid(), "{}", vector["name"]);
    }
    for vector in vectors["invalidReplies"].as_array().ok_or("Missing invalid replies")? {
        let decoded =
            serde_json::from_value::<wire::CanvasSurfaceViewportReply>(vector["value"].clone());
        assert!(!decoded.is_ok_and(|reply| reply.valid()), "{}", vector["name"]);
    }
    let discovery: wire::CanvasSurfaceCapabilitiesReply =
        serde_json::from_value(vectors["capabilities"].clone())?;
    assert!(discovery.valid());
    Ok(())
}

#[test]
fn viewport_centers_preserve_finite_image_relative_points_in_the_reply()
-> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let request = request(&vectors["validRequests"][0])?;
    let state = CanvasViewportState::new(CanvasViewportMode::Custom, 2.0, 1000, 800)?;
    assert_eq!(state.center(), None);
    for center in [[0.0, 1.0], [0.25, 0.75], [1.0, 0.0], [-3.25, 2.75], [f64::MIN, f64::MAX]] {
        let centered = state.with_center(center[0], center[1])?;
        assert_eq!(centered.center(), Some(center));
        let reply: Value = serde_json::from_slice(&request.viewport_reply(&centered, "")?)?;
        assert_eq!(reply["viewport"]["center"], json!({"x":center[0], "y":center[1]}));
    }
    for (x, y) in [(f64::NEG_INFINITY, 0.5), (0.5, f64::NAN), (f64::NAN, 0.5), (0.5, f64::INFINITY)]
    {
        assert!(state.with_center(x, y).is_err());
    }
    let reply: Value = serde_json::from_slice(&request.viewport_reply(&state, "")?)?;
    assert!(reply["viewport"].get("center").is_none());
    Ok(())
}
