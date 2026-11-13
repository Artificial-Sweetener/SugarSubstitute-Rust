// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the context request, capability forms and cancellation against shared vectors.

use super::*;

const CONTEXT: &str =
    include_str!("../../../../../../Sugar/integrations/contracts/canvas_context_vectors.json");
const CAPABILITIES: &str =
    include_str!("../../../../../../Sugar/integrations/contracts/canvas_capability_vectors.json");

#[test]
fn context_matches_shared_request_and_display_only_replies() -> TestResult {
    let vectors = vectors(CONTEXT)?;
    let request = CanvasContextRequest::decode(&bytes(&vectors["request"])?)?;
    assert_eq!(request.request_id(), 7);
    assert_eq!(request.session().as_str(), SESSION);
    for (display_output, key) in [(true, "available"), (false, "unavailable")] {
        let capabilities = CanvasNativeCapabilities { display_output, ..Default::default() };
        let actual: Value = serde_json::from_slice(request.reply(capabilities, None)?.as_bytes())?;
        assert_eq!(actual, vectors[key]);
    }
    for value in vectors["invalidRequests"].as_array().ok_or("Invalid vectors")? {
        assert!(CanvasContextRequest::decode(&bytes(value)?).is_err());
    }
    let cancelled = request.envelope().cancelled_reply()?;
    assert_eq!(cancelled.request_id(), 7);
    let cancelled: Value = serde_json::from_slice(cancelled.as_bytes())?;
    assert_eq!(cancelled, vectors["cancelled"]);
    assert!(accepted(&cancelled, wire::CanvasErrorReply::valid));
    Ok(())
}

#[test]
fn every_capability_form_encodes_exactly_and_rejections_hold() -> TestResult {
    let vectors = vectors(CAPABILITIES)?;
    for form in ["layer", "geometry", "editing"] {
        assert!(accepted(&vectors["valid"][form], wire::CanvasReadReply::valid), "{form}");
    }
    for (name, _, value) in rejections(&vectors)? {
        assert!(!accepted(&value, wire::CanvasReadReply::valid), "{name} was accepted");
    }
    let request = CanvasContextRequest::decode(&bytes(&context_request())?)?;
    let encode =
        |capabilities, document: Option<&CanvasDocument>| -> Result<Value, Box<dyn Error>> {
            Ok(serde_json::from_slice(request.reply(capabilities, document)?.as_bytes())?)
        };
    let display = CanvasNativeCapabilities { display_output: true, ..Default::default() };
    assert_eq!(encode(display, None)?, vectors["valid"]["layer"]);
    let editing = CanvasNativeCapabilities {
        display_output: true,
        geometry: true,
        editing: true,
        workflow_surfaces: true,
    };
    assert_eq!(encode(editing, Some(&document()?))?, vectors["valid"]["editing"]);
    let geometry = CanvasNativeCapabilities { geometry: true, ..display };
    assert_eq!(
        encode(geometry, None)?["context"]["canvas"]["capabilities"],
        json!({"displayOutput": true, "insertImageLayer": false, "positionImageLayer": false,
            "geometry": true})
    );
    // Either editing flag selects the complete form so every flag is stated.
    let workflow_only = CanvasNativeCapabilities { workflow_surfaces: true, ..display };
    let encoded = encode(workflow_only, None)?;
    assert_eq!(encoded["context"]["canvas"]["capabilities"]["editing"], false);
    assert_eq!(encoded["context"]["canvas"]["capabilities"]["geometry"], false);
    assert!(accepted(&encoded, wire::CanvasReadReply::valid));
    Ok(())
}

#[test]
fn context_cancellation_retains_its_original_correlation_and_error_contract() -> TestResult {
    let request = CanvasContextRequest::decode(br#"{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":2147483647,"type":"context.request"}"#)?;
    let encoded = request.envelope().cancelled_reply()?.into_bytes();
    let mut reply: wire::CanvasErrorReply = serde_json::from_slice(&encoded)?;
    assert_eq!(reply.session, request.session().as_str());
    assert_eq!(reply.request_id, request.request_id());
    assert_eq!(reply.failure.status, "cancelled");
    assert!(reply.failure.message.is_empty());
    assert!(reply.valid());
    assert!(CanvasContextRequest::decode(&encoded).is_err());
    reply.failure.status = "unknown".into();
    assert!(!reply.valid());
    reply.failure.status = "cancelled".into();
    reply.failure.message = "🌈".repeat(256);
    assert!(reply.valid());
    reply.failure.message.push('x');
    assert!(!reply.valid());
    let duplicate = String::from_utf8(encoded)?
        .replace("\"requestId\":2147483647", "\"requestId\":2147483647,\"requestId\":1");
    assert!(serde_json::from_str::<wire::CanvasErrorReply>(&duplicate).is_err());
    Ok(())
}

#[test]
fn context_rejects_duplicate_fields_and_excessive_payloads() {
    let duplicate = br#"{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":7,"requestId":8,"type":"context.request"}"#;
    assert!(CanvasContextRequest::decode(duplicate).is_err());
    assert!(matches!(
        CanvasContextRequest::decode(&vec![b' '; MAX_CANVAS_CONTEXT_REQUEST_BYTES + 1]),
        Err(CanvasNativeError::TooLarge)
    ));
}

#[test]
fn generated_reply_requires_an_explicit_null_document() -> TestResult {
    let vectors = vectors(CONTEXT)?;
    let mut reply = vectors["unavailable"].clone();
    reply["context"].as_object_mut().ok_or("Invalid reply context")?.remove("document");
    assert!(serde_json::from_value::<wire::CanvasReadReply>(reply).is_err());
    Ok(())
}
