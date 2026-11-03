// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise geometry commands and replies against the generated schema validators.

use super::*;

fn document_json() -> Value {
    json!({"id": "input:surface-1", "geometryRevision": "geometry-1", "width": 1024, "height": 768})
}

#[test]
fn context_and_resize_decode_and_rejections_hold() -> TestResult {
    let context = CanvasGeometryRequest::decode(&bytes(&request(
        "geometry.request",
        json!({"type": "context"}),
    ))?)?;
    assert_eq!(context.command(), &CanvasGeometryCommand::Context);
    let resize = request(
        "geometry.request",
        json!({"type": "resize", "document": document_json(), "size": {"width": 512, "height": 300000}}),
    );
    assert_eq!(
        CanvasGeometryRequest::decode(&bytes(&resize)?)?.command(),
        &CanvasGeometryCommand::Resize {
            document: document()?,
            size: CanvasDimensions::new(512, 300_000)?
        }
    );
    for (path, value) in [
        ("/command/size/width", json!(0)),
        ("/command/size/height", json!(300_001)),
        ("/command/document/width", json!(0)),
        ("/command/document/id", json!("x".repeat(129))),
        ("/command/type", json!("crop")),
        ("/requestId", json!(0)),
    ] {
        let mut invalid = resize.clone();
        *invalid.pointer_mut(path).ok_or("Missing path")? = value;
        assert!(CanvasGeometryRequest::decode(&bytes(&invalid)?).is_err(), "{path}");
    }
    let mut extended = resize;
    extended["command"]["anchor"] = json!("center");
    assert!(CanvasGeometryRequest::decode(&bytes(&extended)?).is_err());
    Ok(())
}

#[test]
fn geometry_replies_are_schema_valid_and_exact() -> TestResult {
    let request = CanvasGeometryRequest::decode(&bytes(&request(
        "geometry.request",
        json!({"type": "context"}),
    ))?)?;
    let selection = CanvasSelectionBounds::new(-300_000, 4, CanvasDimensions::new(10, 20)?)?;
    let context = CanvasGeometryContext {
        supported: true,
        resize: false,
        selection_supported: true,
        document: Some(document()?),
        selection: Some(selection),
    };
    let encoded: Value = serde_json::from_slice(request.context_reply(&context)?.as_bytes())?;
    assert!(accepted(&encoded, wire::CanvasGeometryReply::valid));
    assert_eq!(
        encoded["result"],
        json!({"kind": "context", "supported": true, "resize": false, "selectionSupported": true,
            "document": document_json(),
            "selection": {"left": -300000, "top": 4, "width": 10, "height": 20}})
    );
    let resized: Value = serde_json::from_slice(request.resized_reply(&document()?)?.as_bytes())?;
    assert_eq!(resized["type"], "geometry.reply");
    assert_eq!(resized["result"], json!({"kind": "resized", "document": document_json()}));
    let empty = CanvasGeometryContext {
        supported: false,
        resize: false,
        selection_supported: false,
        document: None,
        selection: None,
    };
    let encoded: Value = serde_json::from_slice(request.context_reply(&empty)?.as_bytes())?;
    assert_eq!(encoded["result"]["document"], Value::Null);
    assert_eq!(encoded["result"]["selection"], Value::Null);
    assert!(CanvasSelectionBounds::new(300_001, 0, CanvasDimensions::new(1, 1)?).is_err());
    Ok(())
}
