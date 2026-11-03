// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the native protocol against the vectors SugarCubes tests its own side with.

mod context;
mod edit;
mod geometry;
mod source;
mod workflow;

use super::*;
use serde_json::{Value, json};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";

/// Parse one vendored vector file.
fn vectors(text: &str) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(text)?)
}

/// Apply one shared rejection mutation: set `value` at `path`, or delete the final member.
fn mutated(vectors: &Value, mutation: &Value) -> Result<Value, Box<dyn Error>> {
    let base = mutation["base"].as_str().ok_or("Mutation names no base")?;
    let mut value = vectors["valid"][base].clone();
    let path = mutation["path"].as_array().ok_or("Mutation names no path")?;
    let (last, parents) = path.split_last().ok_or("Mutation path is empty")?;
    let mut target = &mut value;
    for step in parents {
        target = match step {
            Value::String(key) => target.get_mut(key.as_str()),
            Value::Number(index) => {
                target.get_mut(usize::try_from(index.as_u64().ok_or("Bad index")?)?)
            }
            _ => None,
        }
        .ok_or("Mutation path does not exist")?;
    }
    let key = last.as_str().ok_or("Mutation leaf is not a member")?;
    let object = target.as_object_mut().ok_or("Mutation parent is not an object")?;
    if mutation["delete"].as_bool() == Some(true) {
        object.remove(key).ok_or("Deleted member does not exist")?;
    } else {
        object.insert(key.to_owned(), mutation["value"].clone());
    }
    Ok(value)
}

/// One named shared rejection: its name, the valid vector it mutates, and the mutated message.
type Rejection = (String, String, Value);

/// Every invalid mutation of the vector file.
fn rejections(vectors: &Value) -> Result<Vec<Rejection>, Box<dyn Error>> {
    vectors["invalid"]
        .as_array()
        .ok_or("Vectors list no rejections")?
        .iter()
        .map(|mutation| {
            Ok((
                mutation["name"].as_str().ok_or("Unnamed mutation")?.to_owned(),
                mutation["base"].as_str().ok_or("Mutation names no base")?.to_owned(),
                mutated(vectors, mutation)?,
            ))
        })
        .collect()
}

/// Whether a host-direction message satisfies the generated shape and validator.
fn accepted<T: serde::de::DeserializeOwned>(value: &Value, valid: impl Fn(&T) -> bool) -> bool {
    serde_json::from_value::<T>(value.clone()).is_ok_and(|decoded| valid(&decoded))
}

fn bytes(value: &Value) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(serde_json::to_vec(value)?)
}

fn request(kind: &str, command: Value) -> Value {
    json!({
        "protocol": "sugarcubes.canvas", "version": 3, "session": SESSION, "requestId": 7,
        "type": kind, "command": command,
    })
}

fn context_request() -> Value {
    json!({
        "protocol": "sugarcubes.canvas", "version": 3, "session": SESSION, "requestId": 7,
        "type": "context.request",
    })
}

fn document() -> Result<CanvasDocument, Box<dyn Error>> {
    Ok(CanvasDocument {
        id: CanvasDocumentId::new("input:surface-1")?,
        geometry_revision: CanvasRevision::new("geometry-1")?,
        size: CanvasDimensions::new(1024, 768)?,
    })
}

fn session() -> Result<PanelSession, Box<dyn Error>> {
    Ok(PanelSession::new(SESSION.into())?)
}

#[test]
fn dispatch_selects_each_family_and_refuses_unknown_types() -> TestResult {
    let hover = json!({"type": "hover", "workflow": "workflow-none", "surface": "s", "mask": null});
    let decoded = [
        CanvasNativeRequest::decode(&bytes(&context_request())?)?,
        CanvasNativeRequest::decode(&bytes(&request(
            "source.request",
            json!({"type": "context"}),
        ))?)?,
        CanvasNativeRequest::decode(&bytes(&request(
            "geometry.request",
            json!({"type": "context"}),
        ))?)?,
        CanvasNativeRequest::decode(&bytes(&request("edit.request", json!({"type": "state"})))?)?,
        CanvasNativeRequest::decode(&bytes(&request("workflow.request", hover))?)?,
    ];
    assert!(matches!(decoded[0], CanvasNativeRequest::Context(_)));
    assert!(matches!(decoded[1], CanvasNativeRequest::Source(_)));
    assert!(matches!(decoded[2], CanvasNativeRequest::Geometry(_)));
    assert!(matches!(decoded[3], CanvasNativeRequest::Edit(_)));
    assert!(matches!(decoded[4], CanvasNativeRequest::Workflow(_)));
    for request in &decoded {
        assert_eq!(request.session().as_str(), SESSION);
        assert_eq!(request.request_id(), 7);
    }
    for kind in ["surface.prepare", "source.reply", "edit.changed", "cancel", "masks.request"] {
        let value = request(kind, json!({"type": "context"}));
        assert!(matches!(
            CanvasNativeRequest::decode(&bytes(&value)?),
            Err(CanvasNativeError::Invalid)
        ));
    }
    let duplicate = br#"{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":7,"type":"edit.request","type":"edit.request","command":{"type":"state"}}"#;
    assert!(CanvasNativeRequest::decode(duplicate).is_err());
    Ok(())
}

#[test]
fn refusals_use_each_family_reply_and_context_stays_display_only() -> TestResult {
    let unsupported = CanvasFailure::new(CanvasFailureStatus::Unsupported);
    let activate = json!({"type": "activate", "workflow": "w", "route": "input", "surface": null, "mask": null});
    for (kind, command, reply) in [
        ("source.request", json!({"type": "watch", "enabled": true}), "source.reply"),
        ("geometry.request", json!({"type": "context"}), "geometry.reply"),
        ("edit.request", json!({"type": "commit"}), "edit.reply"),
        ("workflow.request", activate, "workflow.reply"),
    ] {
        let request = CanvasNativeRequest::decode(&bytes(&request(kind, command))?)?;
        let payload = request.refusal_reply(&unsupported)?;
        assert_eq!(payload.request_id(), 7);
        assert_eq!(payload.session().as_str(), SESSION);
        let value: Value = serde_json::from_slice(payload.as_bytes())?;
        assert_eq!(value["type"], reply);
        assert_eq!(
            value["result"],
            json!({"kind": "failure", "status": "unsupported", "message": ""})
        );
        let cancelled: Value =
            serde_json::from_slice(request.envelope().cancelled_reply()?.as_bytes())?;
        assert_eq!(cancelled["type"], "error.reply");
        assert_eq!(cancelled["failure"]["status"], "cancelled");
    }
    let context = CanvasNativeRequest::decode(&bytes(&context_request())?)?;
    let value: Value = serde_json::from_slice(context.refusal_reply(&unsupported)?.as_bytes())?;
    assert_eq!(value["context"]["canvas"]["capabilities"]["displayOutput"], false);
    assert_eq!(value["context"]["document"], Value::Null);
    Ok(())
}

#[test]
fn every_family_enforces_its_byte_bound_before_parsing() -> TestResult {
    let padded = |value: &Value, total: usize| -> Result<Vec<u8>, Box<dyn Error>> {
        let mut encoded = bytes(value)?;
        encoded.resize(total, b' ');
        Ok(encoded)
    };
    let source = request("source.request", json!({"type": "context"}));
    let geometry = request("geometry.request", json!({"type": "context"}));
    let edit = request("edit.request", json!({"type": "state"}));
    let workflow =
        request("workflow.request", json!({"type": "sync", "workflow": "w", "surfaces": []}));
    for (value, bound) in [
        (&context_request(), MAX_CANVAS_CONTEXT_REQUEST_BYTES),
        (&source, MAX_CANVAS_SOURCE_REQUEST_BYTES),
        (&geometry, MAX_CANVAS_GEOMETRY_REQUEST_BYTES),
        (&edit, MAX_CANVAS_EDIT_REQUEST_BYTES),
        (&workflow, MAX_CANVAS_WORKFLOW_REQUEST_BYTES),
    ] {
        assert!(CanvasNativeRequest::decode(&padded(value, bound)?).is_ok());
        assert!(matches!(
            CanvasNativeRequest::decode(&padded(value, bound + 1)?),
            Err(CanvasNativeError::TooLarge)
        ));
    }
    Ok(())
}

#[test]
fn bounded_values_count_utf16_and_parse_exact_spellings() -> TestResult {
    assert!(CanvasDocumentId::new("🌈".repeat(64)).is_ok());
    assert!(CanvasDocumentId::new(format!("{}x", "🌈".repeat(64))).is_err());
    assert!(CanvasLabel::new("x".repeat(256)).is_ok());
    assert!(CanvasLabel::new("x".repeat(257)).is_err());
    let color = CanvasColor::parse("#3FA9f5")?;
    assert_eq!((color.red, color.green, color.blue), (0x3f, 0xa9, 0xf5));
    assert_eq!(color.to_hex(), "#3fa9f5");
    for spelling in ["red", "#12345g", "#1234567", "#12345", "3fa9f5", "#+1+2+3", "#１２３４５６"]
    {
        assert!(CanvasColor::parse(spelling).is_err(), "{spelling} parsed");
    }
    assert!(CanvasOpacity::new(0.0).is_ok() && CanvasOpacity::new(1.0).is_ok());
    for value in [-0.1, 1.5, f64::NAN, f64::INFINITY] {
        assert!(CanvasOpacity::new(value).is_err());
    }
    assert!(CanvasDimensions::new(300_000, 1).is_ok());
    assert!(CanvasDimensions::new(0, 1).is_err() && CanvasDimensions::new(1, 300_001).is_err());
    Ok(())
}
