// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove native and web surface message admission and exact reply correlation.

use super::*;
use crate::canvas_surface::{CanvasViewportMode, CanvasViewportState};
use serde_json::{Value, json};

fn vectors() -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_message_vectors.json"
    )))?)
}

#[test]
fn surface_requests_match_the_shared_web_vectors() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let base = vectors.get("base").ok_or("Missing surface message base")?;
    let cases = vectors.get("cases").and_then(Value::as_array).ok_or("Missing message cases")?;
    for case in cases {
        let name = case.get("name").and_then(Value::as_str).ok_or("Missing vector name")?;
        let mut value = base.clone();
        if let Some(path) = case.get("path").and_then(Value::as_str) {
            let parts: Vec<_> = path.split('.').collect();
            let (key, ancestors) = parts.split_last().ok_or("Empty vector path")?;
            let mut parent = &mut value;
            for part in ancestors {
                parent = parent.get_mut(*part).ok_or("Missing vector parent")?;
            }
            let object = parent.as_object_mut().ok_or("Vector parent is not an object")?;
            if case.get("remove").and_then(Value::as_bool) == Some(true) {
                object.remove(*key).ok_or("Missing vector field")?;
            } else {
                object.insert((*key).to_owned(), case.get("value").ok_or("Missing value")?.clone());
            }
        }
        let expected = case.get("valid").and_then(Value::as_bool).ok_or("Missing result")?;
        assert_eq!(
            CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?).is_ok(),
            expected,
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn selected_output_reply_retains_original_attachment_and_revision() -> Result<(), Box<dyn Error>> {
    let value = vectors()?.get("base").ok_or("Missing surface message base")?.clone();
    let CanvasSurfaceMessage::Request(request) =
        CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?
    else {
        return Err("Expected a selection request".into());
    };
    assert_eq!(request.request_id(), 1);
    assert_eq!(request.revision(), 1);
    assert_eq!(request.address().view_id(), "view:one");
    let CanvasSurfaceOperation::Select { item_id, file, comparison, inspection } =
        request.operation()
    else {
        return Err("Expected selected output".into());
    };
    assert!(comparison.is_none());
    assert!(inspection.is_none());
    assert_eq!(item_id, "result:one");
    assert_eq!(file.filename(), "résumé🌈.png");
    let reply: Value =
        serde_json::from_slice(&request.reply(CanvasSurfaceReplyStatus::Applied, "")?)?;
    assert_eq!(reply["type"], "surface.reply");
    assert_eq!(reply["address"], value["address"]);
    assert_eq!(reply["revision"], value["revision"]);
    assert_eq!(reply["requestId"], value["requestId"]);
    assert!(request.reply(CanvasSurfaceReplyStatus::Ready, "").is_err());
    assert!(request.reply(CanvasSurfaceReplyStatus::Error, &"x".repeat(513)).is_err());
    Ok(())
}

#[test]
fn a_comparison_selection_decodes_both_results_and_wipes_report_both_extents()
-> Result<(), Box<dyn Error>> {
    let mut value = vectors()?.get("base").ok_or("Missing surface message base")?.clone();
    value["comparison"] = json!({
        "itemId":"result:two", "file":{"filename":"two.png","subfolder":"","type":"output"}
    });
    let CanvasSurfaceMessage::Request(request) =
        CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?
    else {
        return Err("Expected a selection request".into());
    };
    let CanvasSurfaceOperation::Select { comparison: Some(comparison), .. } = request.operation()
    else {
        return Err("Expected a comparison selection".into());
    };
    assert_eq!(comparison.item_id, "result:two");
    assert_eq!(comparison.file.filename(), "two.png");
    let state = CanvasViewportState::new(CanvasViewportMode::Fit, 0.5, 64, 32)?
        .with_secondary_dimensions(16, 24)?;
    assert_eq!(state.secondary_dimensions(), Some((16, 24)));
    let wire = serde_json::to_value(state.to_wire())?;
    assert_eq!(wire["secondaryDimensions"], json!({"width":16,"height":24}));
    assert!(state.with_secondary_dimensions(0, 24).is_err());
    Ok(())
}

#[test]
fn preparation_and_cancellation_keep_the_original_session_identity() -> Result<(), Box<dyn Error>> {
    let mut value = vectors()?.get("base").ok_or("Missing surface message base")?.clone();
    let object = value.as_object_mut().ok_or("Message is not an object")?;
    object.insert("type".into(), json!("surface.prepare"));
    object.remove("itemId");
    object.remove("file");
    let CanvasSurfaceMessage::Request(request) =
        CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?
    else {
        return Err("Expected preparation".into());
    };
    assert!(matches!(request.operation(), CanvasSurfaceOperation::Prepare));
    assert!(request.reply(CanvasSurfaceReplyStatus::Ready, "").is_ok());
    assert!(request.reply(CanvasSurfaceReplyStatus::Applied, "").is_err());
    let cancel = json!({"protocol":"sugarcubes.canvas", "version":3, "session":"canvas-surface-session", "requestId":1, "type":"cancel"});
    let CanvasSurfaceMessage::Cancel { session, request_id } =
        CanvasSurfaceMessage::decode(&serde_json::to_vec(&cancel)?)?
    else {
        return Err("Expected cancellation".into());
    };
    assert_eq!(session.as_str(), "canvas-surface-session");
    assert_eq!(request_id, request.request_id());
    Ok(())
}

#[test]
fn duplicate_fields_and_oversized_messages_are_rejected() -> Result<(), Box<dyn Error>> {
    let value = vectors()?.get("base").ok_or("Missing surface message base")?.clone();
    let encoded = serde_json::to_string(&value)?;
    let duplicate = encoded.replace("\"revision\":1", "\"revision\":1,\"revision\":2");
    assert_ne!(duplicate, encoded);
    assert!(CanvasSurfaceMessage::decode(duplicate.as_bytes()).is_err());
    assert!(matches!(
        CanvasSurfaceMessage::decode(&vec![b' '; MAX_SURFACE_MESSAGE_BYTES + 1]),
        Err(CanvasSurfaceMessageError::TooLarge)
    ));
    Ok(())
}

#[test]
fn overview_requests_match_the_shared_vectors_including_identity_and_size_bounds()
-> Result<(), Box<dyn Error>> {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_overview_vectors.json"
    )))?;
    assert_eq!(vectors["messageBytes"], MAX_SURFACE_MESSAGE_BYTES);
    for case in vectors["validRequests"].as_array().ok_or("Missing valid requests")? {
        let name = case["name"].as_str().ok_or("Missing vector name")?;
        let CanvasSurfaceMessage::Request(request) =
            CanvasSurfaceMessage::decode(&serde_json::to_vec(&case["value"])?)?
        else {
            return Err(format!("{name}: expected an overview request").into());
        };
        let CanvasSurfaceOperation::Overview(cells) = request.operation() else {
            return Err(format!("{name}: expected overview cells").into());
        };
        let expected = case["value"]["cells"].as_array().ok_or("Missing cells")?;
        assert_eq!(cells.len(), expected.len(), "{name}");
        for (cell, wire) in cells.iter().zip(expected) {
            assert_eq!(cell.item_id, wire["itemId"].as_str().ok_or("Missing item")?, "{name}");
            assert_eq!(cell.rect[2], wire["rect"]["width"].as_f64().ok_or("Missing width")?);
        }
    }
    for section in ["invalidRequests", "semanticallyInvalidRequests", "oversizedRequests"] {
        for case in vectors[section].as_array().ok_or("Missing rejected requests")? {
            let name = case["name"].as_str().ok_or("Missing vector name")?;
            assert!(
                CanvasSurfaceMessage::decode(&serde_json::to_vec(&case["value"])?).is_err(),
                "{section}: {name}"
            );
        }
    }
    Ok(())
}

#[test]
fn geometry_occlusion_requests_match_the_shared_vectors() -> Result<(), Box<dyn Error>> {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_occlusion_vectors.json"
    )))?;
    assert_eq!(vectors["maxOcclusions"], crate::canvas_surface::MAX_GEOMETRY_OCCLUSIONS);
    for case in vectors["validRequests"].as_array().ok_or("Missing valid requests")? {
        let name = case["name"].as_str().ok_or("Missing vector name")?;
        let CanvasSurfaceMessage::Request(request) =
            CanvasSurfaceMessage::decode(&serde_json::to_vec(&case["value"])?)?
        else {
            return Err(format!("{name}: expected a geometry request").into());
        };
        let CanvasSurfaceOperation::Geometry(geometry) = request.operation() else {
            return Err(format!("{name}: expected geometry").into());
        };
        let wire = &case["value"]["geometry"];
        let expected: Vec<_> = wire["occlusions"]
            .as_array()
            .map(|rects| {
                rects
                    .iter()
                    .map(|rect| {
                        ["x", "y", "width", "height"]
                            .map(|key| rect[key].as_f64().unwrap_or(f64::NAN))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let decoded: Vec<_> = geometry.occlusions().map(|(x, y, w, h)| [x, y, w, h]).collect();
        assert_eq!(decoded, expected, "{name}");
        assert_eq!(geometry.modal(), wire["modal"].as_bool().unwrap_or(false), "{name}");
    }
    for case in vectors["invalidRequests"].as_array().ok_or("Missing invalid requests")? {
        let name = case["name"].as_str().ok_or("Missing vector name")?;
        assert!(
            CanvasSurfaceMessage::decode(&serde_json::to_vec(&case["value"])?).is_err(),
            "{name}"
        );
    }
    Ok(())
}
